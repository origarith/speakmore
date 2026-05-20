import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import { ChevronDown, Globe } from "lucide-react";
import type { ModelCardStatus } from "@/components/onboarding";
import { ModelCard } from "@/components/onboarding";
import { Alert } from "@/components/ui";
import { useModelStore } from "@/stores/modelStore";
import { useSettings } from "@/hooks/useSettings";
import { LANGUAGES } from "@/lib/constants/languages.ts";
import {
  groupModelsBySeries,
  sortModelsForDisplay,
} from "@/lib/utils/modelSeries";
import type { ModelInfo } from "@/bindings";
import { AsrProviderSettings } from "./AsrProviderSettings";
import { ModelAdvancedSettingsDialog } from "./ModelAdvancedSettingsDialog";

const BUILT_IN_LOCAL_PROVIDER_ID = "built_in_local";
const ALIYUN_REALTIME_PROVIDER_ID = "aliyun_qwen3_asr_realtime";
const ALIYUN_DEFAULT_MODEL = "qwen3-asr-flash";

// check if model supports a language based on its supported_languages list
const modelSupportsLanguage = (model: ModelInfo, langCode: string): boolean => {
  return model.supported_languages.includes(langCode);
};

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [languageFilter, setLanguageFilter] = useState("all");
  const [languageDropdownOpen, setLanguageDropdownOpen] = useState(false);
  const [languageSearch, setLanguageSearch] = useState("");
  const [advancedSettingsModel, setAdvancedSettingsModel] =
    useState<ModelInfo | null>(null);
  const languageDropdownRef = useRef<HTMLDivElement>(null);
  const languageSearchInputRef = useRef<HTMLInputElement>(null);
  const { settings } = useSettings();
  const {
    models,
    currentModel,
    downloadingModels,
    downloadProgress,
    downloadStats,
    verifyingModels,
    extractingModels,
    loading,
    downloadModel,
    cancelDownload,
    selectModel,
    deleteModel,
  } = useModelStore();
  const activeAsrProviderId =
    settings?.asr_provider_id ?? BUILT_IN_LOCAL_PROVIDER_ID;
  const isCloudAsrActive = activeAsrProviderId !== BUILT_IN_LOCAL_PROVIDER_ID;
  const activeCloudAsrModel =
    settings?.asr_models?.[activeAsrProviderId] ??
    (activeAsrProviderId === ALIYUN_REALTIME_PROVIDER_ID
      ? "qwen3-asr-flash-realtime-2026-02-10"
      : ALIYUN_DEFAULT_MODEL);

  // click outside handler for language dropdown
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        languageDropdownRef.current &&
        !languageDropdownRef.current.contains(event.target as Node)
      ) {
        setLanguageDropdownOpen(false);
        setLanguageSearch("");
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // focus search input when dropdown opens
  useEffect(() => {
    if (languageDropdownOpen && languageSearchInputRef.current) {
      languageSearchInputRef.current.focus();
    }
  }, [languageDropdownOpen]);

  // filtered languages for dropdown (exclude "auto")
  const filteredLanguages = useMemo(() => {
    return LANGUAGES.filter(
      (lang) =>
        lang.value !== "auto" &&
        lang.label.toLowerCase().includes(languageSearch.toLowerCase()),
    );
  }, [languageSearch]);

  // Get selected language label
  const selectedLanguageLabel = useMemo(() => {
    if (languageFilter === "all") {
      return t("settings.models.filters.allLanguages");
    }
    return LANGUAGES.find((lang) => lang.value === languageFilter)?.label || "";
  }, [languageFilter, t]);

  const getModelStatus = (modelId: string): ModelCardStatus => {
    if (modelId in extractingModels) {
      return "extracting";
    }
    if (modelId in verifyingModels) {
      return "verifying";
    }
    if (modelId in downloadingModels) {
      return "downloading";
    }
    if (switchingModelId === modelId) {
      return "switching";
    }
    if (modelId === currentModel) {
      return "active";
    }
    const model = models.find((m: ModelInfo) => m.id === modelId);
    if (model?.is_downloaded) {
      return "available";
    }
    return "downloadable";
  };

  const getDownloadProgress = (modelId: string): number | undefined => {
    const progress = downloadProgress[modelId];
    return progress?.percentage;
  };

  const getDownloadSpeed = (modelId: string): number | undefined => {
    const stats = downloadStats[modelId];
    return stats?.speed;
  };

  const handleModelSelect = async (modelId: string) => {
    setSwitchingModelId(modelId);
    try {
      await selectModel(modelId);
    } finally {
      setSwitchingModelId(null);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    await downloadModel(modelId);
  };

  const handleModelDelete = async (modelId: string) => {
    const model = models.find((m: ModelInfo) => m.id === modelId);
    const modelName = model?.name || modelId;
    const isActive = modelId === currentModel;

    const confirmed = await ask(
      isActive
        ? t("settings.models.deleteActiveConfirm", { modelName })
        : t("settings.models.deleteConfirm", { modelName }),
      {
        title: t("settings.models.deleteTitle"),
        kind: "warning",
      },
    );

    if (confirmed) {
      try {
        await deleteModel(modelId);
      } catch (err) {
        console.error(`Failed to delete model ${modelId}:`, err);
      }
    }
  };

  const handleModelCancel = async (modelId: string) => {
    try {
      await cancelDownload(modelId);
    } catch (err) {
      console.error(`Failed to cancel download for ${modelId}:`, err);
    }
  };

  const handleAdvancedSettings = (modelId: string) => {
    const model = models.find((item: ModelInfo) => item.id === modelId);
    if (model) {
      setAdvancedSettingsModel(model);
    }
  };

  // Filter models based on language filter
  const filteredModels = useMemo(() => {
    return models.filter((model: ModelInfo) => {
      if (languageFilter !== "all") {
        if (!modelSupportsLanguage(model, languageFilter)) return false;
      }
      return true;
    });
  }, [models, languageFilter]);

  const modelSeriesGroups = useMemo(() => {
    return groupModelsBySeries(
      sortModelsForDisplay(filteredModels, currentModel),
    );
  }, [filteredModels, currentModel]);

  if (loading) {
    return (
      <div className="mx-auto w-full max-w-5xl">
        <div className="flex items-center justify-center py-16">
          <div className="w-8 h-8 border-2 border-logo-primary border-t-transparent rounded-full animate-spin" />
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-5xl space-y-5">
      <AsrProviderSettings />

      <Alert variant={isCloudAsrActive ? "info" : "success"}>
        {isCloudAsrActive
          ? t("settings.models.asr.activeCloudSummary", {
              model: activeCloudAsrModel || ALIYUN_DEFAULT_MODEL,
            })
          : t("settings.models.asr.activeLocalSummary")}
      </Alert>

      <div className="mb-4">
        <h1 className="mb-2 text-xl font-semibold">
          {t("settings.models.title")}
        </h1>
        <p className="text-sm text-text/60">
          {isCloudAsrActive
            ? t("settings.models.asr.localModelsInactive")
            : t("settings.models.description")}
        </p>
      </div>
      <div className="flex justify-end">
        <div className="relative" ref={languageDropdownRef}>
          <button
            type="button"
            onClick={() => setLanguageDropdownOpen(!languageDropdownOpen)}
            className={`flex min-h-9 items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm font-medium transition-colors ${
              languageFilter !== "all"
                ? "border-logo-primary/20 bg-logo-primary/10 text-logo-primary"
                : "frost-control text-text/60 hover:border-logo-primary/30"
            }`}
          >
            <Globe className="h-3.5 w-3.5" />
            <span className="max-w-[140px] truncate">
              {selectedLanguageLabel}
            </span>
            <ChevronDown
              className={`h-3.5 w-3.5 transition-transform ${
                languageDropdownOpen ? "rotate-180" : ""
              }`}
            />
          </button>

          {languageDropdownOpen && (
            <div className="frost-menu absolute right-0 top-full z-50 mt-1 w-56 overflow-hidden rounded-lg">
              <div className="border-b border-frost-border p-2">
                <input
                  ref={languageSearchInputRef}
                  type="text"
                  value={languageSearch}
                  onChange={(e) => setLanguageSearch(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && filteredLanguages.length > 0) {
                      setLanguageFilter(filteredLanguages[0].value);
                      setLanguageDropdownOpen(false);
                      setLanguageSearch("");
                    } else if (e.key === "Escape") {
                      setLanguageDropdownOpen(false);
                      setLanguageSearch("");
                    }
                  }}
                  placeholder={t("settings.general.language.searchPlaceholder")}
                  className="frost-control w-full rounded-md border px-2 py-1 text-sm focus:outline-none focus:ring-2 focus:ring-logo-primary/20"
                />
              </div>
              <div className="max-h-48 overflow-y-auto">
                <button
                  type="button"
                  onClick={() => {
                    setLanguageFilter("all");
                    setLanguageDropdownOpen(false);
                    setLanguageSearch("");
                  }}
                  className={`w-full px-3 py-1.5 text-left text-sm transition-colors ${
                    languageFilter === "all"
                      ? "bg-logo-primary/10 font-semibold text-logo-primary"
                      : "hover:bg-white/50 dark:hover:bg-white/10"
                  }`}
                >
                  {t("settings.models.filters.allLanguages")}
                </button>
                {filteredLanguages.map((lang) => (
                  <button
                    key={lang.value}
                    type="button"
                    onClick={() => {
                      setLanguageFilter(lang.value);
                      setLanguageDropdownOpen(false);
                      setLanguageSearch("");
                    }}
                    className={`w-full px-3 py-1.5 text-left text-sm transition-colors ${
                      languageFilter === lang.value
                        ? "bg-logo-primary/10 font-semibold text-logo-primary"
                        : "hover:bg-white/50 dark:hover:bg-white/10"
                    }`}
                  >
                    {lang.label}
                  </button>
                ))}
                {filteredLanguages.length === 0 && (
                  <div className="px-3 py-2 text-center text-sm text-text/50">
                    {t("settings.general.language.noResults")}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {filteredModels.length > 0 ? (
        <div className="space-y-7">
          {modelSeriesGroups.map((group) => {
            const downloadedCount = group.models.filter(
              (model) => model.is_downloaded || model.is_custom,
            ).length;

            return (
              <section key={group.definition.id} className="space-y-3">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h2 className="text-base font-semibold text-text">
                        {t(group.definition.titleKey)}
                      </h2>
                      <span className="rounded-md border border-frost-border bg-white/30 px-2 py-1 text-xs font-semibold text-text/50 dark:bg-white/10">
                        {group.models.length}
                      </span>
                    </div>
                    <p className="mt-1 max-w-2xl text-sm text-text/50">
                      {t(group.definition.descriptionKey)}
                    </p>
                  </div>
                  {downloadedCount > 0 && (
                    <span className="text-xs font-medium text-text/50">
                      {downloadedCount} {t("settings.models.downloaded")}
                    </span>
                  )}
                </div>
                <div className="grid gap-3 lg:grid-cols-2">
                  {group.models.map((model: ModelInfo) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      status={getModelStatus(model.id)}
                      onSelect={handleModelSelect}
                      onDownload={handleModelDownload}
                      onDelete={handleModelDelete}
                      onCancel={handleModelCancel}
                      onAdvancedSettings={handleAdvancedSettings}
                      downloadProgress={getDownloadProgress(model.id)}
                      downloadSpeed={getDownloadSpeed(model.id)}
                      showRecommended={false}
                    />
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      ) : (
        <div className="py-8 text-center text-text/50">
          {t("settings.models.noModelsMatch")}
        </div>
      )}
      {advancedSettingsModel && (
        <ModelAdvancedSettingsDialog
          model={advancedSettingsModel}
          onClose={() => setAdvancedSettingsModel(null)}
        />
      )}
    </div>
  );
};
