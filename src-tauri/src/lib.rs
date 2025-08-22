// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn check_emby_server(ip: String) -> Result<bool, String> {
    let url = if ip.starts_with("http") {
        format!("{}/System/Info/Public", ip)
    } else {
        format!("http://{}:8096/System/Info/Public", ip)
    };

    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        Err(e) => {
            eprintln!("Error connecting to Emby server: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
async fn find_emby_on_tailscale() -> Result<Option<String>, String> {
    // Get Tailscale status to find connected devices
    let tailscale_ips = get_tailscale_ips().await?;
    
    if tailscale_ips.is_empty() {
        return Err("No Tailscale devices found. Make sure Tailscale is running and connected.".to_string());
    }

    // Test each IP to see if it's running an Emby server
    for ip in tailscale_ips {
        let url = format!("http://{}:8096/System/Info/Public", ip);
        
        // Use a shorter timeout for discovery to make it faster
        match timeout(Duration::from_secs(3), 
            reqwest::Client::new()
                .get(&url)
                .timeout(Duration::from_secs(2))
                .send()
        ).await {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    println!("Found Emby server at: {}", ip);
                    return Ok(Some(ip));
                }
            }
            Ok(Err(e)) => {
                println!("Error checking {}: {}", ip, e);
            }
            Err(_) => {
                println!("Timeout checking {}", ip);
            }
        }
    }

    Ok(None)
}

async fn get_tailscale_ips() -> Result<Vec<String>, String> {
    // Try to get Tailscale status using the CLI from various common paths
    let tailscale_paths = vec![
        "tailscale",                                    // System PATH
        "/usr/bin/tailscale",                          // Linux/macOS system install
        "/usr/local/bin/tailscale",                    // Homebrew on macOS
        "/opt/homebrew/bin/tailscale",                 // Homebrew on Apple Silicon
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale", // macOS App Store version
    ];

    let mut last_error = String::new();
    
    for path in tailscale_paths {
        // First, try a simple status command to see if tailscale is working
        match Command::new(path)
            .args(["status"])
            .output()
        {
            Ok(simple_output) => {
                if !simple_output.status.success() {
                    let stderr = String::from_utf8_lossy(&simple_output.stderr);
                    println!("Simple tailscale status failed from {}: {}", path, stderr);
                    last_error = format!("Tailscale not ready from {}: {}", path, stderr);
                    continue;
                }
                
                // If simple status works, try with JSON
                match Command::new(path)
                    .args(["status", "--json"])
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stdout_trimmed = stdout.trim();
                            
                            // Debug: print the actual output
                            println!("Tailscale status output length: {}", stdout_trimmed.len());
                            if stdout_trimmed.is_empty() {
                                println!("Tailscale status returned empty output");
                                last_error = format!("Tailscale status returned empty output from {}", path);
                                continue;
                            }
                            
                            // Validate JSON before parsing
                            if !stdout_trimmed.starts_with('{') && !stdout_trimmed.starts_with('[') {
                                println!("Tailscale status output is not valid JSON: {}", stdout_trimmed);
                                last_error = format!("Tailscale status output is not valid JSON from {}: {}", path, stdout_trimmed);
                                continue;
                            }
                            
                            // Parse the JSON output to extract IP addresses
                            return match parse_tailscale_status(&stdout_trimmed) {
                                Ok(ips) => Ok(ips),
                                Err(e) => Err(format!("Failed to parse Tailscale status from {}: {}", path, e))
                            };
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            last_error = format!("Tailscale JSON command failed from {}: {}", path, stderr);
                        }
                    }
                    Err(e) => {
                        last_error = format!("Failed to run tailscale JSON command from {}: {}", path, e);
                        continue;
                    }
                }
            }
            Err(e) => {
                last_error = format!("Failed to run tailscale from {}: {}", path, e);
                continue;
            }
        }
    }

    // If all paths failed, try fallback method
    println!("Tailscale CLI not found, trying fallback method...");
    get_tailscale_ips_fallback().await.or_else(|fallback_error| {
        Err(format!(
            "Tailscale CLI not found in common locations and fallback failed. \
            Last CLI error: {}. Fallback error: {}. \
            Please ensure Tailscale is installed and running.",
            last_error, fallback_error
        ))
    })
}

fn parse_tailscale_status(json_str: &str) -> Result<Vec<String>, String> {
    if json_str.is_empty() {
        return Err("Empty JSON string".to_string());
    }
    
    let json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse JSON: {}. Content: '{}'", e, json_str))?;

    let mut ips = Vec::new();

    // Debug: Print the structure to understand what we're working with
    println!("JSON structure keys: {:?}", json.as_object().map(|obj| obj.keys().collect::<Vec<_>>()));

    // Extract IPs from the peer list
    if let Some(peer_obj) = json.get("Peer") {
        if let Some(peers) = peer_obj.as_object() {
            println!("Found {} peers in Tailscale status", peers.len());
            for (peer_id, peer_data) in peers {
                println!("Checking peer: {}", peer_id);
                
                // Try different possible field names for IP addresses
                let ip_fields = ["TailscaleIPs", "Addrs", "Endpoints", "PrimaryRoutes"];
                
                for field in ip_fields.iter() {
                    if let Some(ip_data) = peer_data.get(field) {
                        extract_ips_from_value(ip_data, &mut ips, &format!("peer {} {}", peer_id, field));
                    }
                }
            }
        }
    }

    // Also check if there's a "Self" entry for the current device
    if let Some(self_data) = json.get("Self") {
        println!("Checking self data");
        let ip_fields = ["TailscaleIPs", "Addrs", "Endpoints"];
        
        for field in ip_fields.iter() {
            if let Some(ip_data) = self_data.get(field) {
                extract_ips_from_value(ip_data, &mut ips, &format!("self {}", field));
            }
        }
    }

    // Alternative: try to find any field that looks like it contains IPs
    if ips.is_empty() {
        find_ips_in_json_recursively(&json, &mut ips);
    }

    println!("Found {} Tailscale IPs: {:?}", ips.len(), ips);
    
    if ips.is_empty() {
        return Err("No Tailscale IPs found in the status output".to_string());
    }
    
    Ok(ips)
}

fn extract_ips_from_value(value: &serde_json::Value, ips: &mut Vec<String>, context: &str) {
    match value {
        serde_json::Value::Array(array) => {
            for item in array {
                if let Some(ip_str) = item.as_str() {
                    if let Some(ip) = extract_ip_from_string(ip_str) {
                        println!("Found IP from {} array: {}", context, ip);
                        ips.push(ip);
                    }
                }
            }
        }
        serde_json::Value::String(s) => {
            if let Some(ip) = extract_ip_from_string(s) {
                println!("Found IP from {} string: {}", context, ip);
                ips.push(ip);
            }
        }
        _ => {}
    }
}

fn extract_ip_from_string(ip_str: &str) -> Option<String> {
    // Remove any CIDR notation (e.g., "/32") and port numbers
    let ip = ip_str.split('/').next()?.split(':').next()?;
    
    // Validate it looks like a Tailscale IP (100.x.x.x range)
    if ip.starts_with("100.") {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            if let (Ok(first), Ok(second), Ok(_third), Ok(_fourth)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
                parts[3].parse::<u8>()
            ) {
                if first == 100 && (64..=127).contains(&second) {
                    return Some(ip.to_string());
                }
            }
        }
    }
    
    None
}

fn find_ips_in_json_recursively(value: &serde_json::Value, ips: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(ip) = extract_ip_from_string(s) {
                println!("Found IP recursively: {}", ip);
                ips.push(ip);
            }
        }
        serde_json::Value::Array(array) => {
            for item in array {
                find_ips_in_json_recursively(item, ips);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_, val) in obj {
                find_ips_in_json_recursively(val, ips);
            }
        }
        _ => {}
    }
}

async fn get_tailscale_ips_fallback() -> Result<Vec<String>, String> {
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::net::TcpStream;
    
    // Common Tailscale IP ranges to scan
    // Tailscale uses 100.x.x.x range (100.64.0.0/10)
    let mut candidate_ips = Vec::new();
    
    // Check if we can find network interfaces with Tailscale IPs
    // This is a simplified approach - scan some common ranges
    let base_ranges = [
        "100.64", "100.65", "100.66", "100.67", "100.68", "100.69",
        "100.70", "100.71", "100.72", "100.73", "100.74", "100.75",
        "100.100", "100.101", "100.102", "100.103", "100.104", "100.105",
    ];
    
    println!("Scanning common Tailscale IP ranges...");
    
    // Quick scan of the local subnet to find potential Tailscale devices
    for base in base_ranges.iter().take(6) { // Limit to first 6 ranges for performance
        for last_octet in [1, 2, 3, 4, 5, 10, 20, 50, 100] { // Common device IPs
            let ip = format!("{}.0.{}", base, last_octet);
            if let Ok(addr) = ip.parse::<IpAddr>() {
                // Quick connectivity test
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    TcpStream::connect((addr, 22)) // SSH port as connectivity test
                ).await {
                    Ok(Ok(_)) => {
                        println!("Found potential device at: {}", ip);
                        candidate_ips.push(ip);
                    },
                    _ => {} // Ignore timeouts and connection failures
                }
            }
        }
    }
    
    if candidate_ips.is_empty() {
        // If no devices found in quick scan, try a broader approach
        // Look for the current machine's Tailscale IP by checking network interfaces
        if let Ok(local_tailscale_ip) = get_local_tailscale_ip() {
            // Extract the network portion and scan nearby IPs
            if let Ok(addr) = local_tailscale_ip.parse::<Ipv4Addr>() {
                let octets = addr.octets();
                let base_ip = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
                
                for i in 1..=10 {
                    if i != octets[3] { // Skip our own IP
                        candidate_ips.push(format!("{}.{}", base_ip, i));
                    }
                }
            }
        }
    }
    
    if candidate_ips.is_empty() {
        return Err("No Tailscale devices found. Make sure you're connected to Tailscale.".to_string());
    }
    
    println!("Found {} potential Tailscale devices to check", candidate_ips.len());
    Ok(candidate_ips)
}

fn get_local_tailscale_ip() -> Result<String, String> {
    // Try to find local Tailscale IP by checking network interfaces
    // This is a simplified version - in a real implementation you'd use system APIs
    use std::process::Command;
    
    // Try to use ifconfig/ip command to find Tailscale interface
    let commands = [
        ("ifconfig", vec![]),
        ("ip", vec!["addr", "show"]),
    ];
    
    for (cmd, args) in commands.iter() {
        if let Ok(output) = Command::new(cmd).args(args).output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                
                // Look for lines containing Tailscale IP (100.x.x.x)
                for line in output_str.lines() {
                    if let Some(ip) = extract_tailscale_ip_from_line(line) {
                        return Ok(ip);
                    }
                }
            }
        }
    }
    
    Err("Could not find local Tailscale IP".to_string())
}

fn extract_tailscale_ip_from_line(line: &str) -> Option<String> {
    // Look for IP addresses in the 100.x.x.x range
    // Simple pattern matching without regex
    let words: Vec<&str> = line.split_whitespace().collect();
    
    for word in words {
        // Remove common prefixes and suffixes
        let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        
        if cleaned.starts_with("100.") {
            let parts: Vec<&str> = cleaned.split('.').collect();
            if parts.len() == 4 {
                // Validate it's a proper IP in Tailscale range
                if let (Ok(first), Ok(second), Ok(third), Ok(fourth)) = (
                    parts[0].parse::<u8>(),
                    parts[1].parse::<u8>(),
                    parts[2].parse::<u8>(),
                    parts[3].parse::<u8>()
                ) {
                    if first == 100 && (64..=127).contains(&second) {
                        return Some(format!("{}.{}.{}.{}", first, second, third, fourth));
                    }
                }
            }
        }
    }
    
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, check_emby_server, find_emby_on_tailscale])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
