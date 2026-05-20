import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type ModelInfo } from "@/bindings";
import type { ModelCardStatus } from "./ModelCard";
import ModelCard from "./ModelCard";
import SpeakMoreLogo from "../icons/SpeakMoreLogo";
import { useModelStore } from "../../stores/modelStore";
import { Alert, Button, Input } from "@/components/ui";
import { useSettings } from "@/hooks/useSettings";
import {
  groupModelsBySeries,
  sortModelsForDisplay,
} from "@/lib/utils/modelSeries";

const ALIYUN_PROVIDER_ID = "aliyun_qwen3_asr_flash";
const ALIYUN_DEFAULT_MODEL = "qwen3-asr-flash";

interface OnboardingProps {
  onModelSelected: () => void;
}

const Onboarding: React.FC<OnboardingProps> = ({ onModelSelected }) => {
  const { t } = useTranslation();
  const {
    models,
    downloadModel,
    selectModel,
    downloadingModels,
    verifyingModels,
    extractingModels,
    downloadProgress,
    downloadStats,
  } = useModelStore();
  const { setAsrProvider, updateAsrApiKey, updateAsrModel } = useSettings();
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [dashscopeKey, setDashscopeKey] = useState("");
  const [isConfiguringCloud, setIsConfiguringCloud] = useState(false);

  const isDownloading = selectedModelId !== null;

  // Watch for the selected model to finish downloading + verifying + extracting
  useEffect(() => {
    if (!selectedModelId) return;

    const model = models.find((m) => m.id === selectedModelId);
    const stillDownloading = selectedModelId in downloadingModels;
    const stillVerifying = selectedModelId in verifyingModels;
    const stillExtracting = selectedModelId in extractingModels;

    if (
      model?.is_downloaded &&
      !stillDownloading &&
      !stillVerifying &&
      !stillExtracting
    ) {
      // Model is ready — select it and transition
      selectModel(selectedModelId).then((success) => {
        if (success) {
          onModelSelected();
        } else {
          toast.error(t("onboarding.errors.selectModel"));
          setSelectedModelId(null);
        }
      });
    }
  }, [
    selectedModelId,
    models,
    downloadingModels,
    verifyingModels,
    extractingModels,
    selectModel,
    onModelSelected,
  ]);

  const handleDownloadModel = async (modelId: string) => {
    setSelectedModelId(modelId);

    // Error toast is handled centrally by the model-download-failed event listener
    // in modelStore — no toast here to avoid duplicates.
    const success = await downloadModel(modelId);
    if (!success) {
      setSelectedModelId(null);
    }
  };

  const handleUseCloudAsr = async () => {
    setIsConfiguringCloud(true);
    try {
      const trimmedKey = dashscopeKey.trim();
      if (trimmedKey) {
        await updateAsrApiKey(ALIYUN_PROVIDER_ID, trimmedKey);
      }
      await updateAsrModel(ALIYUN_PROVIDER_ID, ALIYUN_DEFAULT_MODEL);

      const status = await commands.getAsrProviderStatus(ALIYUN_PROVIDER_ID);
      if (status.status !== "ok" || !status.data.configured) {
        toast.error(t("onboarding.cloudAsr.errors.missingKey"));
        return;
      }

      await setAsrProvider(ALIYUN_PROVIDER_ID);
      onModelSelected();
    } finally {
      setIsConfiguringCloud(false);
    }
  };

  const getModelStatus = (modelId: string): ModelCardStatus => {
    if (modelId in extractingModels) return "extracting";
    if (modelId in verifyingModels) return "verifying";
    if (modelId in downloadingModels) return "downloading";
    return "downloadable";
  };

  const getModelDownloadProgress = (modelId: string): number | undefined => {
    return downloadProgress[modelId]?.percentage;
  };

  const getModelDownloadSpeed = (modelId: string): number | undefined => {
    return downloadStats[modelId]?.speed;
  };

  const downloadableModelGroups = useMemo(() => {
    const downloadableModels = models.filter(
      (model: ModelInfo) => !model.is_downloaded,
    );
    return groupModelsBySeries(sortModelsForDisplay(downloadableModels));
  }, [models]);

  return (
    <div className="app-shell inset-0 flex h-screen w-screen flex-col gap-4 p-6">
      <div className="flex shrink-0 flex-col items-center gap-2">
        <SpeakMoreLogo
          brandName={t("app.name")}
          tagline={t("app.taglineShort")}
        />
        <p className="text-text/70 max-w-md font-medium mx-auto">
          {t("onboarding.subtitle")}
        </p>
      </div>

      <div className="mx-auto flex min-h-0 w-full max-w-5xl flex-1 flex-col text-center">
        <div className="flex flex-col gap-5 overflow-y-auto pb-6 pr-1">
          <div className="warm-panel space-y-3 p-4 text-start">
            <div>
              <h2 className="text-sm font-semibold">
                {t("onboarding.cloudAsr.title")}
              </h2>
              <p className="text-sm text-text/60 mt-1">
                {t("onboarding.cloudAsr.description")}
              </p>
            </div>
            <Alert variant="info">{t("onboarding.cloudAsr.notice")}</Alert>
            <div className="flex gap-2">
              <Input
                type="password"
                value={dashscopeKey}
                onChange={(event) => setDashscopeKey(event.target.value)}
                placeholder={t("onboarding.cloudAsr.apiKeyPlaceholder")}
                className="flex-1"
                disabled={isConfiguringCloud || isDownloading}
              />
              <Button
                onClick={handleUseCloudAsr}
                disabled={isConfiguringCloud || isDownloading}
              >
                {isConfiguringCloud
                  ? t("onboarding.cloudAsr.configuring")
                  : t("onboarding.cloudAsr.useCloud")}
              </Button>
            </div>
          </div>

          <div className="text-xs font-medium text-text/50 uppercase tracking-wide">
            {t("onboarding.localModelsDivider")}
          </div>

          {downloadableModelGroups.map((group) => (
            <section key={group.definition.id} className="space-y-3 text-start">
              <div>
                <h2 className="text-base font-semibold text-text">
                  {t(group.definition.titleKey)}
                </h2>
                <p className="mt-1 text-sm text-text/55">
                  {t(group.definition.descriptionKey)}
                </p>
              </div>
              <div className="grid gap-3 lg:grid-cols-2">
                {group.models.map((model: ModelInfo) => (
                  <ModelCard
                    key={model.id}
                    model={model}
                    variant={model.is_recommended ? "featured" : "default"}
                    status={getModelStatus(model.id)}
                    disabled={isDownloading}
                    onSelect={handleDownloadModel}
                    onDownload={handleDownloadModel}
                    downloadProgress={getModelDownloadProgress(model.id)}
                    downloadSpeed={getModelDownloadSpeed(model.id)}
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
};

export default Onboarding;
