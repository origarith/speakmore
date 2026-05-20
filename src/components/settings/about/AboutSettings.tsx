import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import SpeakMoreLogo from "../../icons/SpeakMoreLogo";
import { AppDataDirectory } from "../AppDataDirectory";
import { AppLanguageSelector } from "../AppLanguageSelector";
import { LogDirectory } from "../debug";

const SOURCE_CODE_URL = "https://github.com/OrigArith/SpeakMore";

export const AboutSettings: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const brandCredit = t("settings.about.brand.credit");

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6">
      <div className="warm-panel p-5">
        <div className="flex items-center justify-between gap-4">
          <SpeakMoreLogo
            brandName={t("app.name")}
            tagline={t("app.taglineShort")}
          />
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="rounded-full border border-frost-border bg-white/40 px-3 py-1 text-xs font-mono text-text/60 dark:bg-white/10">
            v{version}
          </span>
        </div>
        <p className="mt-4 text-sm leading-relaxed text-text/70">
          {t("settings.about.brand.description")}
        </p>
        {brandCredit && (
          <p className="mt-2 text-xs font-medium text-text/50">{brandCredit}</p>
        )}
      </div>

      <SettingsGroup title={t("settings.about.title")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped={true} />
        <SettingContainer
          title={t("settings.about.version.title")}
          description={t("settings.about.version.description")}
          grouped={true}
        >
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="text-sm font-mono">v{version}</span>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.sourceCode.title")}
          description={t("settings.about.sourceCode.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => openUrl(SOURCE_CODE_URL)}
          >
            {t("settings.about.sourceCode.button")}
          </Button>
        </SettingContainer>

        <AppDataDirectory descriptionMode="tooltip" grouped={true} />
        <LogDirectory grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.about.acknowledgments.title")}>
        <SettingContainer
          title={t("settings.about.acknowledgments.originalProject.title")}
          description={t(
            "settings.about.acknowledgments.originalProject.description",
          )}
          grouped={true}
          layout="stacked"
        >
          <div className="text-sm text-mid-gray">
            {t("settings.about.acknowledgments.originalProject.details")}
          </div>
        </SettingContainer>
        <SettingContainer
          title={t("settings.about.acknowledgments.whisper.title")}
          description={t("settings.about.acknowledgments.whisper.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="text-sm text-mid-gray">
            {t("settings.about.acknowledgments.whisper.details")}
          </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
