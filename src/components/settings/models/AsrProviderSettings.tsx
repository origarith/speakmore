import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Cloud, HardDrive, ShieldAlert } from "lucide-react";
import { commands, type AsrProviderStatus } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import {
  Alert,
  Button,
  Input,
  Select,
  type SelectOption,
} from "@/components/ui";

const BUILT_IN_LOCAL_PROVIDER_ID = "built_in_local";
const ALIYUN_REALTIME_PROVIDER_ID = "aliyun_qwen3_asr_realtime";

export const AsrProviderSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    settings,
    isUpdating,
    setAsrProvider,
    updateAsrApiKey,
    updateAsrModel,
  } = useSettings();
  const [status, setStatus] = useState<AsrProviderStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [modelInput, setModelInput] = useState("");

  const providers = settings?.asr_providers ?? [];
  const selectedProviderId =
    settings?.asr_provider_id ?? BUILT_IN_LOCAL_PROVIDER_ID;
  const selectedProvider = providers.find(
    (provider) => provider.id === selectedProviderId,
  );
  const isCloudProvider =
    selectedProvider?.kind !== undefined &&
    selectedProvider.kind !== "built_in_local";
  const apiKey = settings?.asr_api_keys?.[selectedProviderId] ?? "";
  const model = settings?.asr_models?.[selectedProviderId] ?? "";
  const modelPlaceholder =
    selectedProviderId === ALIYUN_REALTIME_PROVIDER_ID
      ? "qwen3-asr-flash-realtime-2026-02-10"
      : "qwen3-asr-flash";

  const providerOptions = useMemo<SelectOption[]>(
    () =>
      providers.map((provider) => ({
        value: provider.id,
        label: t(`settings.models.asr.providers.${provider.id}`, {
          defaultValue: provider.label,
        }),
      })),
    [providers, t],
  );

  useEffect(() => {
    setApiKeyInput(apiKey);
    setModelInput(model);
  }, [apiKey, model, selectedProviderId]);

  const refreshStatus = useCallback(
    async (providerId = selectedProviderId) => {
      if (!providerId) return;
      setIsChecking(true);
      setStatusError(null);
      try {
        const result = await commands.getAsrProviderStatus(providerId);
        if (result.status === "ok") {
          setStatus(result.data);
        } else {
          setStatus(null);
          setStatusError(t("settings.models.asr.status.checkFailed"));
        }
      } catch {
        setStatus(null);
        setStatusError(t("settings.models.asr.status.checkFailed"));
      } finally {
        setIsChecking(false);
      }
    },
    [selectedProviderId, t],
  );

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus, settings?.asr_api_keys, settings?.asr_models]);

  const handleProviderChange = async (value: string | null) => {
    if (!value || value === selectedProviderId) return;
    const nextProvider = providers.find((provider) => provider.id === value);
    if (!nextProvider) return;

    setStatus(null);
    setStatusError(null);
    try {
      await setAsrProvider(value);
      await refreshStatus(value);
    } catch {
      setStatusError(t("settings.models.asr.status.providerChangeFailed"));
    }
  };

  const persistPendingCloudSettings = async () => {
    if (!isCloudProvider) return;

    const trimmed = apiKeyInput.trim();
    if (trimmed !== apiKey) {
      await updateAsrApiKey(selectedProviderId, trimmed);
    }

    const trimmedModel = modelInput.trim();
    if (trimmedModel !== model) {
      await updateAsrModel(selectedProviderId, trimmedModel);
    }
  };

  const handleCheckStatus = async () => {
    try {
      await persistPendingCloudSettings();
      await refreshStatus();
    } catch {
      setStatus(null);
      setStatusError(t("settings.models.asr.status.saveFailed"));
    }
  };

  const statusMessage = (() => {
    if (statusError) {
      return (
        <Alert variant="error" className="mt-3">
          {statusError}
        </Alert>
      );
    }

    if (!isCloudProvider) return null;

    if (!status) return null;

    if (!status.configured) {
      return (
        <Alert variant="warning" className="mt-3">
          {t("settings.models.asr.status.missingKey")}
        </Alert>
      );
    }

    return (
      <Alert variant="success" className="mt-3">
        {t("settings.models.asr.status.ready", {
          source: t(
            `settings.models.asr.apiKeySource.${status.api_key_source}`,
          ),
        })}
      </Alert>
    );
  })();

  return (
    <section className="frost-surface space-y-4 rounded-lg p-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-sm font-semibold">
            {t("settings.models.asr.title")}
          </h2>
          <p className="mt-1 text-sm text-text/60">
            {t("settings.models.asr.description")}
          </p>
        </div>
        {isCloudProvider ? (
          <Cloud className="h-5 w-5 shrink-0 text-logo-primary" />
        ) : (
          <HardDrive className="h-5 w-5 shrink-0 text-logo-primary" />
        )}
      </div>

      <div className="grid gap-3">
        <label className="grid gap-1">
          <span className="text-xs font-medium text-text/70">
            {t("settings.models.asr.provider")}
          </span>
          <Select
            value={selectedProviderId}
            options={providerOptions}
            onChange={handleProviderChange}
            isClearable={false}
            disabled={isUpdating("asr_provider_id")}
          />
        </label>

        {isCloudProvider && (
          <>
            <Alert variant="info">
              {t("settings.models.asr.privacyNotice")}
            </Alert>

            <label className="grid gap-1">
              <span className="text-xs font-medium text-text/70">
                {t("settings.models.asr.apiKey")}
              </span>
              <Input
                type="password"
                value={apiKeyInput}
                onChange={(event) => setApiKeyInput(event.target.value)}
                onBlur={() => void persistPendingCloudSettings()}
                disabled={isUpdating(`asr_api_key:${selectedProviderId}`)}
                placeholder={t("settings.models.asr.apiKeyPlaceholder")}
              />
            </label>

            <label className="grid gap-1">
              <span className="text-xs font-medium text-text/70">
                {t("settings.models.asr.model")}
              </span>
              <Input
                value={modelInput}
                onChange={(event) => setModelInput(event.target.value)}
                onBlur={() => void persistPendingCloudSettings()}
                disabled={isUpdating(`asr_model:${selectedProviderId}`)}
                placeholder={t("settings.models.asr.modelPlaceholder", {
                  model: modelPlaceholder,
                })}
              />
            </label>

            <div className="flex items-center justify-between gap-3 rounded-lg border border-frost-border bg-white/40 px-3 py-2 dark:bg-white/5">
              <div className="flex items-center gap-2 text-xs text-text/60">
                <ShieldAlert className="h-4 w-4" />
                <span>{t("settings.models.asr.noFallback")}</span>
              </div>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void handleCheckStatus()}
                disabled={isChecking}
              >
                {isChecking
                  ? t("settings.models.asr.checking")
                  : t("settings.models.asr.checkStatus")}
              </Button>
            </div>

            {statusMessage}
          </>
        )}
      </div>
    </section>
  );
};
