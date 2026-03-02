import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import TrayApp from "./TrayApp";
import BubbleApp from "./BubbleApp";
import "./styles/index.css";

const label = getCurrentWebviewWindow().label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {label === "bubble" ? <BubbleApp /> : <TrayApp />}
  </React.StrictMode>
);
