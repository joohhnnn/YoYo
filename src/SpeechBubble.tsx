import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { SpeechBubbleEvent } from "./types";
import "./styles/index.css";

export default function SpeechBubble() {
  const [text, setText] = useState("");
  const [visible, setVisible] = useState(false);
  const [fading, setFading] = useState(false);
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const hideTimerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    const unlisten = listen<SpeechBubbleEvent>(
      "speech-bubble",
      (event) => {
        // Clear any existing timers
        clearTimeout(fadeTimerRef.current);
        clearTimeout(hideTimerRef.current);

        setText(event.payload.text);
        setVisible(true);
        setFading(false);

        const dismissSecs = event.payload.auto_dismiss_secs || 8;

        // Start fade 1s before hide
        fadeTimerRef.current = setTimeout(
          () => setFading(true),
          (dismissSecs - 1) * 1000
        );

        // Hide and close window
        hideTimerRef.current = setTimeout(() => {
          setVisible(false);
          getCurrentWebviewWindow().hide().catch(() => {});
        }, dismissSecs * 1000);
      }
    );

    return () => {
      unlisten.then((f) => f());
      clearTimeout(fadeTimerRef.current);
      clearTimeout(hideTimerRef.current);
    };
  }, []);

  const dismiss = () => {
    clearTimeout(fadeTimerRef.current);
    clearTimeout(hideTimerRef.current);
    setVisible(false);
    getCurrentWebviewWindow().hide().catch(() => {});
  };

  if (!visible) return null;

  return (
    <div className="w-full h-full" data-tauri-drag-region>
      <div
        onClick={dismiss}
        className={`
          p-3 bg-zinc-900/95 backdrop-blur-xl text-white rounded-xl
          shadow-[0_8px_32px_rgba(0,0,0,0.5)] border border-zinc-700/50
          cursor-pointer max-w-[260px] relative
          transition-opacity duration-1000
          ${fading ? "opacity-0" : "opacity-100"}
        `}
      >
        {/* Triangle pointer (right side) */}
        <div className="absolute -right-2 top-4 w-0 h-0 border-t-[8px] border-b-[8px] border-l-[8px] border-transparent border-l-zinc-900/95" />

        <p className="text-[13px] leading-relaxed">{text}</p>
        <p className="text-[10px] text-zinc-500 mt-1.5">Click to dismiss</p>
      </div>
    </div>
  );
}
