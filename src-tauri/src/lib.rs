// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, check_emby_server])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
