import React from "react";
import { useTranslation } from "react-i18next";
import {
  Cog,
  Cpu,
  FlaskConical,
  History,
  Home,
  Info,
  Sparkles,
} from "lucide-react";
import SpeakMoreLogo from "./icons/SpeakMoreLogo";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: Home,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: () => true,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <aside className="flex h-full w-52 shrink-0 flex-col border-e border-frost-border bg-[var(--color-frost-sidebar)] px-3 backdrop-blur-2xl">
      <div className="flex h-20 items-center px-1">
        <SpeakMoreLogo
          brandName={t("app.name")}
          tagline={t("app.taglineShort")}
        />
      </div>
      <div className="flex w-full flex-col gap-1 border-t border-frost-border pt-3">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <button
              key={section.id}
              type="button"
              className={`group relative flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-start text-sm transition-[background-color,color,box-shadow,transform] duration-200 ${
                isActive
                  ? "bg-logo-primary text-white shadow-[0_10px_22px_rgba(47,143,131,0.2)]"
                  : "text-text/70 hover:bg-white/50 hover:text-text dark:hover:bg-white/10"
              }`}
              onClick={() => onSectionChange(section.id)}
            >
              {isActive && (
                <span className="absolute start-0 top-1/2 h-5 w-1 -translate-y-1/2 rounded-e-full bg-white/85" />
              )}
              <Icon
                width={18}
                height={18}
                className={`shrink-0 transition-colors duration-200 ${
                  isActive
                    ? "text-white"
                    : "text-text/45 group-hover:text-logo-primary"
                }`}
              />
              <span
                className="truncate font-medium"
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </span>
            </button>
          );
        })}
      </div>
    </aside>
  );
};
