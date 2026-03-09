import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import SettingsApp from "./SettingsApp";
import BubbleApp from "./BubbleApp";
import "./styles/index.css";

const label = getCurrentWebviewWindow().label;

function App() {
  if (label === "bubble") return <BubbleApp />;
  return <SettingsApp />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
