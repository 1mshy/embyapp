import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [serverIp, setServerIp] = useState("");
  const [statusMessage, setStatusMessage] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isInitializing, setIsInitializing] = useState(true);
  const [isFindingOnTailscale, setIsFindingOnTailscale] = useState(false);

  // Check for saved server IP on app startup
  useEffect(() => {
    async function initializeApp() {
      try {
        // Try to get saved server IP from localStorage
        const savedIp = localStorage.getItem("emby_server_ip");
        
        if (savedIp) {
          setServerIp(savedIp);
          setStatusMessage("Checking saved server...");
          
          // Check if the saved server is still accessible
          const isServerRunning = await invoke("check_emby_server", { ip: savedIp });
          
          if (isServerRunning) {
            setStatusMessage("Connecting to saved server...");
            // Add protocol if not present
            const url = savedIp.startsWith('http') 
              ? `${savedIp}/web/index.html`
              : `http://${savedIp}:8096/web/index.html`;
            window.location.replace(url);
            return;
          } else {
            setStatusMessage("Saved server not accessible. Please enter a new server IP.");
          }
        } else {
          setStatusMessage("Welcome! Please enter your Emby server IP to get started.");
        }
      } catch (error) {
        console.error("Error initializing app:", error);
        setStatusMessage("Welcome! Please enter your Emby server IP to get started.");
      } finally {
        setIsInitializing(false);
      }
    }

    initializeApp();
  }, []);

  async function connectToServer() {
    if (!serverIp.trim()) {
      setStatusMessage("Please enter a server IP address");
      return;
    }

    setIsLoading(true);
    setStatusMessage("Checking server connection...");

    try {
      const isServerRunning = await invoke("check_emby_server", { ip: serverIp.trim() });
      
      if (isServerRunning) {
        // Save the server IP in localStorage for future use
        localStorage.setItem("emby_server_ip", serverIp.trim());
        setStatusMessage("Server found! Redirecting...");
        
        // Add protocol if not present
        const url = serverIp.startsWith('http') 
          ? `${serverIp}/web/index.html`
          : `http://${serverIp}:8096/web/index.html`;
        window.location.replace(url);
      } else {
        setStatusMessage("Server not accessible. Please check the IP address and try again.");
      }
    } catch (error) {
      setStatusMessage(`Error: ${error}`);
    } finally {
      setIsLoading(false);
    }
  }

  async function findEmbyOnTailscale() {
    setIsFindingOnTailscale(true);
    setStatusMessage("Scanning Tailscale network for Emby servers...");

    try {
      const result = await invoke("find_emby_on_tailscale");
      
      if (result) {
        // Server found, save it and connect
        const foundIp = result as string;
        setServerIp(foundIp);
        localStorage.setItem("emby_server_ip", foundIp);
        setStatusMessage(`Found Emby server at ${foundIp}! Connecting...`);
        
        // Redirect to the found server
        const url = `http://${foundIp}:8096/web/index.html`;
        setTimeout(() => {
          window.location.replace(url);
        }, 1000);
      } else {
        setStatusMessage("No Emby servers found on Tailscale network. Please enter IP manually.");
      }
    } catch (error) {
      let errorMessage = `${error}`;
      
      // Provide helpful guidance based on the error
      if (errorMessage.includes("Tailscale CLI not found")) {
        errorMessage = "Tailscale not detected. Please install Tailscale from tailscale.com and make sure it's running, then try again.";
      } else if (errorMessage.includes("No Tailscale devices found")) {
        errorMessage = "No devices found on Tailscale network. Make sure you're connected to Tailscale and other devices are online.";
      }
      
      setStatusMessage(errorMessage);
    } finally {
      setIsFindingOnTailscale(false);
    }
  }

  if (isInitializing) {
    return (
      <main className="container">
        <h1>Welcome to the unofficial Emby app</h1>
        <p className="status-message">Initializing...</p>
      </main>
    );
  }

  return (
    <main className="container">
      <h1>Welcome to the unofficial Emby app</h1>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          connectToServer();
        }}
      >
        <input
          id="server-input"
          type="text"
          value={serverIp}
          onChange={(e) => setServerIp(e.currentTarget.value)}
          placeholder="Enter Emby server IP (e.g., 192.168.1.100)"
          disabled={isLoading || isFindingOnTailscale}
        />
        <button type="submit" disabled={isLoading || isFindingOnTailscale}>
          {isLoading ? "Checking..." : "Connect"}
        </button>
      </form>
      
      <div className="tailscale-section">
        <p>Or let us find your Emby server automatically:</p>
        <button 
          type="button" 
          className="tailscale-button"
          onClick={findEmbyOnTailscale}
          disabled={isLoading || isFindingOnTailscale}
        >
          {isFindingOnTailscale ? "Scanning Tailscale..." : "🔍 Find on Tailscale"}
        </button>
      </div>
      
      
      {statusMessage && (
        <p className={`status-message ${statusMessage.includes("Error") || statusMessage.includes("not accessible") ? "error" : ""}`}>
          {statusMessage}
        </p>
      )}
    </main>
  );
}

export default App;
