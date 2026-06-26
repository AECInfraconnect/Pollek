import { useMode } from "../../context/ModeContext";
import { useTranslation } from "react-i18next";
import { Settings2 } from "lucide-react";
import { APP_MODES, appModeLabel } from "../../lib/modes";

export function ModeSwitcher({ collapsed }: { collapsed?: boolean }) {
  const { mode, setMode, enterpriseCloudEnabled, cloudModeStatus } = useMode();
  const { t } = useTranslation();
  const title =
    mode === "enterprise_cloud" || enterpriseCloudEnabled
      ? appModeLabel(mode)
      : `${appModeLabel(mode)} - Enterprise Cloud locked until Pollek Cloud is connected`;

  if (collapsed) {
    return (
      <div
        className="flex h-8 w-8 items-center justify-center rounded-lg border border-border bg-card text-muted-foreground"
        title={title}
      >
        <Settings2 className="h-4 w-4" />
      </div>
    );
  }

  return (
    <select
      value={mode}
      onChange={(e) => setMode(e.target.value as any)}
      className="w-full rounded-lg border border-border bg-card px-2 py-1.5 text-sm text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary"
    >
      {APP_MODES.map((appMode) => (
        <option
          key={appMode}
          value={appMode}
          disabled={appMode === "enterprise_cloud" && !enterpriseCloudEnabled}
        >
          {t(`mode.${appMode}`, appModeLabel(appMode))}
          {appMode === "enterprise_cloud" && !enterpriseCloudEnabled
            ? cloudModeStatus === "checking"
              ? " (checking)"
              : " (connect Pollek Cloud)"
            : ""}
        </option>
      ))}
    </select>
  );
}
