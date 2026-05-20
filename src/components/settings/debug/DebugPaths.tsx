import React from "react";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";

interface DebugPathsProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const DebugPaths: React.FC<DebugPathsProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();

  return (
    <SettingContainer
      title={t("settings.debug.paths.title")}
      description={t("settings.debug.paths.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <div className="space-y-2 text-sm text-text/65">
        <div>
          <span className="font-medium">
            {t("settings.debug.paths.appData")}
          </span>{" "}
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-mono text-xs select-text">
            %APPDATA%/app.speakmore.desktop
          </span>
        </div>
        <div>
          <span className="font-medium">
            {t("settings.debug.paths.models")}
          </span>{" "}
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-mono text-xs select-text">
            %APPDATA%/app.speakmore.desktop/models
          </span>
        </div>
        <div>
          <span className="font-medium">
            {t("settings.debug.paths.settings")}
          </span>{" "}
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-mono text-xs select-text">
            %APPDATA%/app.speakmore.desktop/settings_store.json
          </span>
        </div>
      </div>
    </SettingContainer>
  );
};
