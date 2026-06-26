import {
  createContext,
  useCallback,
  useEffect,
  useContext,
  useState,
  type ReactNode,
} from "react";
import { defaultClient } from "../services/api";
import {
  isEnterpriseCloudProfileConnected,
  normalizeAppMode,
  type AppMode,
} from "../lib/modes";
export type { AppMode } from "../lib/modes";

interface ModeCtx {
  mode: AppMode;
  setMode: (m: AppMode) => void;
  enterpriseCloudEnabled: boolean;
  cloudModeStatus: "checking" | "connected" | "locked";
  refreshCloudMode: () => Promise<void>;
}
const Ctx = createContext<ModeCtx>({
  mode: "desktop_simple",
  setMode: () => {},
  enterpriseCloudEnabled: false,
  cloudModeStatus: "checking",
  refreshCloudMode: async () => {},
});

export function ModeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<AppMode>(
    () => normalizeAppMode(localStorage.getItem("pollek.mode")),
  );
  const [cloudModeStatus, setCloudModeStatus] = useState<
    "checking" | "connected" | "locked"
  >("checking");
  const enterpriseCloudEnabled = cloudModeStatus === "connected";

  const refreshCloudMode = useCallback(async () => {
    setCloudModeStatus((current) =>
      current === "connected" ? "connected" : "checking",
    );
    try {
      const profile = await defaultClient.getCloudPdpProfile();
      setCloudModeStatus(
        isEnterpriseCloudProfileConnected(profile) ? "connected" : "locked",
      );
    } catch {
      setCloudModeStatus("locked");
    }
  }, []);

  const setMode = (m: AppMode) => {
    const normalized = normalizeAppMode(m);
    if (normalized === "enterprise_cloud" && !enterpriseCloudEnabled) {
      return;
    }
    localStorage.setItem("pollek.mode", normalized);
    setModeState(normalized);
  };

  useEffect(() => {
    refreshCloudMode();
    const refresh = () => void refreshCloudMode();
    window.addEventListener("focus", refresh);
    window.addEventListener("pollek-cloud-profile-changed", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      window.removeEventListener("pollek-cloud-profile-changed", refresh);
    };
  }, [refreshCloudMode]);

  useEffect(() => {
    if (mode === "enterprise_cloud" && cloudModeStatus === "locked") {
      localStorage.setItem("pollek.mode", "desktop_advanced");
      setModeState("desktop_advanced");
    }
  }, [cloudModeStatus, mode]);

  return (
    <Ctx.Provider
      value={{
        mode,
        setMode,
        enterpriseCloudEnabled,
        cloudModeStatus,
        refreshCloudMode,
      }}
    >
      {children}
    </Ctx.Provider>
  );
}
export const useMode = () => useContext(Ctx);
