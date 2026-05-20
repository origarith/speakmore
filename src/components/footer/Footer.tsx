import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useTranslation } from "react-i18next";
import { Activity, Cloud, HardDrive, ShieldCheck } from "lucide-react";

import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";
import { useSettings } from "@/hooks/useSettings";

const Footer: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const { settings } = useSettings();
  const isCloudAsr =
    settings?.asr_provider_id && settings.asr_provider_id !== "built_in_local";

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
    <footer className="w-full border-t border-frost-border bg-[var(--color-frost-sidebar)]/80 backdrop-blur-2xl">
      <div className="flex min-h-12 items-center justify-between gap-3 px-5 py-2.5 text-xs text-text/60">
        <div className="flex min-w-0 items-center gap-3">
          <span className="inline-flex items-center gap-1.5 rounded-full border border-frost-border bg-white/40 px-2.5 py-1 font-medium dark:bg-white/10">
            <Activity className="h-3.5 w-3.5 text-success" />
            {t("footer.statusReady")}
          </span>
          <span className="hidden items-center gap-1.5 rounded-full border border-frost-border bg-white/40 px-2.5 py-1 font-medium dark:bg-white/10 sm:inline-flex">
            {isCloudAsr ? (
              <Cloud className="h-3.5 w-3.5 text-logo-primary" />
            ) : (
              <HardDrive className="h-3.5 w-3.5 text-logo-primary" />
            )}
            {isCloudAsr ? t("footer.modeCloud") : t("footer.modeLocal")}
          </span>
          <ModelSelector />
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <UpdateChecker />
          <span
            aria-hidden="true"
            className="h-1 w-1 rounded-full bg-text/30"
          />
          <span className="hidden items-center gap-1.5 sm:inline-flex">
            <ShieldCheck className="h-3.5 w-3.5 text-success" />
            {t("footer.privacyFirst")}
          </span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span>v{version}</span>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
