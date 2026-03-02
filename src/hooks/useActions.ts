import { useState, useCallback } from "react";
import { executeAction } from "../services/actions";
import type { SuggestedAction } from "../types";

export function useActions() {
  const [executing, setExecuting] = useState<string | null>(null);

  const execute = useCallback(async (action: SuggestedAction) => {
    setExecuting(action.label);
    try {
      await executeAction(action);
    } catch (e) {
      console.error("Action failed:", e);
    } finally {
      setExecuting(null);
    }
  }, []);

  return { executing, execute };
}
