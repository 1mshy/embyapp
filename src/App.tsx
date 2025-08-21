import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [serverIp, setServerIp] = useState("");
  const [statusMessage, setStatusMessage] = useState("");
  const [isLoading, setIsLoading] = useState(false);

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
          disabled={isLoading}
        />
        <button type="submit" disabled={isLoading}>
          {isLoading ? "Checking..." : "Connect"}
        </button>
      </form>
      
      {statusMessage && (
        <p className={`status-message ${statusMessage.includes("Error") || statusMessage.includes("not accessible") ? "error" : ""}`}>
          {statusMessage}
        </p>
      )}
    </main>
  );
}

export default App;
