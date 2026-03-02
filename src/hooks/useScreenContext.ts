import { useState, useCallback, useRef, useEffect } from "react";
import { analyzeScreen } from "../services/ai";
import type { AnalysisResult } from "../types";

export function useScreenContext() {
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lastAnalysis = useRef<number>(0);

  const analyze = useCallback(async (cooldownMs = 10000) => {
    const now = Date.now();
    if (now - lastAnalysis.current < cooldownMs) {
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const analysis = await analyzeScreen();
      setResult(analysis);
      lastAnalysis.current = Date.now();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Listen for auto-analyze events (from app switch or shortcut)
  useEffect(() => {
    const handler = () => {
      analyze();
    };
    window.addEventListener("yoyo-auto-analyze", handler);
    return () => window.removeEventListener("yoyo-auto-analyze", handler);
  }, [analyze]);

  return { result, loading, error, analyze };
}
