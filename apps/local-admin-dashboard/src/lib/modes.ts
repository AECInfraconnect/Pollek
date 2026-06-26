import type { RuntimeModeV2 } from "../services/types";

export type AppMode = "desktop_simple" | "desktop_advanced" | "enterprise_cloud";

export const APP_MODES: AppMode[] = [
  "desktop_simple",
  "desktop_advanced",
  "enterprise_cloud",
];

export const MODE_LABELS: Record<AppMode, string> = {
  desktop_simple: "Simple",
  desktop_advanced: "Advance",
  enterprise_cloud: "Enterprise Cloud",
};

export function normalizeAppMode(value: string | null | undefined): AppMode {
  if (value === "desktop_advanced" || value === "advanced") {
    return "desktop_advanced";
  }
  if (
    value === "enterprise_cloud" ||
    value === "enterprise" ||
    value === "enterprise_server" ||
    value === "cloud"
  ) {
    return "enterprise_cloud";
  }
  return "desktop_simple";
}

export function appModeLabel(mode: AppMode): string {
  return MODE_LABELS[mode];
}

export function appModeToRuntimeMode(mode: AppMode): RuntimeModeV2 {
  return mode === "enterprise_cloud" ? "enterprise_server" : mode;
}

export function isAdvanceMode(mode: AppMode): boolean {
  return mode === "desktop_advanced" || mode === "enterprise_cloud";
}

export function isEnterpriseCloudMode(mode: AppMode): boolean {
  return mode === "enterprise_cloud";
}

export function isEnterpriseCloudProfileConnected(profile: {
  status?: string;
  tenant_id?: string;
  device_id?: string;
  pdp_endpoint?: string;
} | null | undefined): boolean {
  return Boolean(
    profile?.status === "connected" &&
      profile.tenant_id &&
      profile.device_id &&
      profile.pdp_endpoint,
  );
}
