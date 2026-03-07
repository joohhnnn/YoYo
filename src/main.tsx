import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import TrayApp from "./TrayApp";
import BubbleApp from "./BubbleApp";
import SpeechBubble from "./SpeechBubble";
import "./styles/index.css";

const label = getCurrentWebviewWindow().label;

function App() {
  if (label === "bubble") return <BubbleApp />;
  if (label === "speech-bubble") return <SpeechBubble />;
  return <TrayApp />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
