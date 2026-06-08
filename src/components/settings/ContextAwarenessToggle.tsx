import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { useOsType } from "../../hooks/useOsType";
import { Alert, SettingContainer } from "../ui";
import Badge from "../ui/Badge";

interface ContextAwarenessToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ContextAwarenessToggle: React.FC<ContextAwarenessToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const osType = useOsType();
    const isMac = osType === "macos";
    const enabled = getSetting("context_awareness_enabled") || false;
    const updating = isUpdating("context_awareness_enabled");

    if (!isMac) {
      return (
        <SettingContainer
          title={t("settings.advanced.contextAwareness.label")}
          description={t("settings.advanced.contextAwareness.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
          disabled
        >
          <div className="space-y-2">
            <Badge variant="secondary">
              {t("settings.advanced.contextAwareness.macOnly")}
            </Badge>
            <p className="text-sm text-text/55">
              {t("settings.advanced.contextAwareness.unavailable")}
            </p>
          </div>
        </SettingContainer>
      );
    }

    return (
      <SettingContainer
        title={t("settings.advanced.contextAwareness.label")}
        description={t("settings.advanced.contextAwareness.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <div className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <Badge variant="secondary">
              {t("settings.advanced.contextAwareness.macOnly")}
            </Badge>
            <label
              className={`inline-flex items-center ${
                updating ? "cursor-not-allowed" : "cursor-pointer"
              }`}
            >
              <input
                type="checkbox"
                className="sr-only peer"
                checked={enabled}
                disabled={updating}
                onChange={(event) =>
                  updateSetting(
                    "context_awareness_enabled",
                    event.target.checked,
                  )
                }
              />
              <div
                className={`relative h-6 w-11 shrink-0 rounded-full border transition-[background-color,border-color,box-shadow] duration-200 ${
                  enabled
                    ? "border-background-ui bg-background-ui shadow-[0_6px_16px_rgba(47,143,131,0.22)]"
                    : "border-black/10 bg-black/10 shadow-inner dark:border-white/10 dark:bg-white/10"
                } peer-focus:ring-2 peer-focus:ring-logo-primary/25 peer-disabled:opacity-50`}
              >
                <span
                  aria-hidden="true"
                  className="absolute top-1/2 h-5 w-5 -translate-y-1/2 rounded-full border border-black/10 bg-white shadow-sm transition-[inset-inline-start,box-shadow,transform] duration-200"
                  style={{ insetInlineStart: enabled ? "22px" : "2px" }}
                />
              </div>
            </label>
          </div>
          <Alert variant="warning" contained className="rounded-md px-3 py-2">
            {t("settings.advanced.contextAwareness.privacyWarning")}
          </Alert>
        </div>
      </SettingContainer>
    );
  });
