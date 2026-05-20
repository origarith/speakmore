import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { commands, type PostProcessProvider } from "@/bindings";
import type { ModelOption } from "./types";
import type { DropdownOption } from "../../ui/Dropdown";

export type PostProcessProviderState = {
  providerOptions: DropdownOption[];
  selectedProviderId: string;
  selectedProvider: PostProcessProvider | undefined;
  isCustomProvider: boolean;
  isAppleProvider: boolean;
  isDeepSeekProvider: boolean;
  appleIntelligenceUnavailable: boolean;
  configurationError: string;
  clearConfigurationError: () => void;
  baseUrl: string;
  handleBaseUrlChange: (value: string) => void;
  handleBaseUrlBlur: () => void;
  isBaseUrlUpdating: boolean;
  apiKey: string;
  handleApiKeyChange: (value: string) => void;
  handleApiKeyBlur: () => void;
  isApiKeyUpdating: boolean;
  model: string;
  handleModelChange: (value: string) => void;
  modelOptions: ModelOption[];
  isModelUpdating: boolean;
  reasoningEffort: string;
  reasoningOptions: DropdownOption[];
  handleReasoningEffortChange: (value: string) => void;
  isReasoningUpdating: boolean;
  isFetchingModels: boolean;
  handleProviderSelect: (providerId: string) => void;
  handleModelSelect: (value: string) => void;
  handleModelCreate: (value: string) => void;
  handleRefreshModels: () => Promise<string[]>;
  savePendingChanges: () => Promise<boolean>;
};

const APPLE_PROVIDER_ID = "apple_intelligence";
const DEEPSEEK_PROVIDER_ID = "deepseek";

export const usePostProcessProviderState = (): PostProcessProviderState => {
  const { t } = useTranslation();
  const {
    settings,
    isUpdating,
    setPostProcessProvider,
    updatePostProcessBaseUrl,
    updatePostProcessApiKey,
    updatePostProcessModel,
    updatePostProcessReasoningEffort,
    fetchPostProcessModels,
    postProcessModelOptions,
  } = useSettings();

  // Settings are guaranteed to have providers after migration
  const providers = settings?.post_process_providers || [];

  const selectedProviderId = useMemo(() => {
    return settings?.post_process_provider_id || providers[0]?.id || "openai";
  }, [providers, settings?.post_process_provider_id]);

  const selectedProvider = useMemo(() => {
    return (
      providers.find((provider) => provider.id === selectedProviderId) ||
      providers[0]
    );
  }, [providers, selectedProviderId]);

  const isAppleProvider = selectedProvider?.id === APPLE_PROVIDER_ID;
  const isDeepSeekProvider = selectedProvider?.id === DEEPSEEK_PROVIDER_ID;
  const [appleIntelligenceUnavailable, setAppleIntelligenceUnavailable] =
    useState(false);
  const [configurationError, setConfigurationError] = useState("");

  const persistedBaseUrl = selectedProvider?.base_url ?? "";
  const persistedApiKey =
    settings?.post_process_api_keys?.[selectedProviderId] ?? "";
  const model = settings?.post_process_models?.[selectedProviderId] ?? "";
  const [draftBaseUrl, setDraftBaseUrl] = useState(persistedBaseUrl);
  const [draftApiKey, setDraftApiKey] = useState(persistedApiKey);
  const reasoningEffort =
    settings?.post_process_reasoning_efforts?.[selectedProviderId] ??
    (isDeepSeekProvider ? "disabled" : "");

  useEffect(() => {
    setDraftBaseUrl(persistedBaseUrl);
    setDraftApiKey(persistedApiKey);
  }, [persistedApiKey, persistedBaseUrl, selectedProviderId]);

  const providerOptions = useMemo<DropdownOption[]>(() => {
    return providers.map((provider) => ({
      value: provider.id,
      label: provider.label,
    }));
  }, [providers]);

  const handleProviderSelect = useCallback(
    async (providerId: string) => {
      // Clear error state on any selection attempt (allows dismissing the error)
      setAppleIntelligenceUnavailable(false);
      setConfigurationError("");

      if (providerId === selectedProviderId) return;

      // Check Apple Intelligence availability before selecting
      if (providerId === APPLE_PROVIDER_ID) {
        const available = await commands.checkAppleIntelligenceAvailable();
        if (!available) {
          setAppleIntelligenceUnavailable(true);
          // Don't return - still set the provider so dropdown shows the selection
          // The backend gracefully handles unavailable Apple Intelligence
        }
      }

      try {
        await setPostProcessProvider(providerId);
      } catch (error) {
        setConfigurationError(String(error));
        return;
      }

      // Auto-fetch available models for the new provider so the model dropdown
      // reflects what's actually valid. Without this, a stale model value from
      // a previous provider/base_url can persist and silently 404 at runtime.
      // Skip when the provider isn't configured yet (no API key / empty base URL)
      // to avoid unnecessary backend errors.
      if (providerId !== APPLE_PROVIDER_ID) {
        const provider = providers.find((p) => p.id === providerId);
        const apiKey = settings?.post_process_api_keys?.[providerId] ?? "";
        const hasBaseUrl = (provider?.base_url ?? "").trim() !== "";
        const hasApiKey = apiKey.trim() !== "";

        if (provider?.id === "custom" ? hasBaseUrl : hasApiKey) {
          void fetchPostProcessModels(providerId).catch((error) => {
            setConfigurationError(String(error));
          });
        }
      }
    },
    [
      selectedProviderId,
      setPostProcessProvider,
      fetchPostProcessModels,
      providers,
      settings,
    ],
  );

  const persistBaseUrl = useCallback(
    async (value: string) => {
      if (!selectedProvider || selectedProvider.id !== "custom") {
        return;
      }
      const trimmed = value.trim();
      if (trimmed && trimmed !== persistedBaseUrl) {
        setConfigurationError("");
        await updatePostProcessBaseUrl(selectedProvider.id, trimmed);
      }
    },
    [persistedBaseUrl, selectedProvider, updatePostProcessBaseUrl],
  );

  const persistApiKey = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      if (trimmed !== persistedApiKey) {
        setConfigurationError("");
        await updatePostProcessApiKey(selectedProviderId, trimmed);
      }
    },
    [persistedApiKey, selectedProviderId, updatePostProcessApiKey],
  );

  const handleBaseUrlChange = useCallback((value: string) => {
    setDraftBaseUrl(value);
  }, []);

  const handleBaseUrlBlur = useCallback(() => {
    void persistBaseUrl(draftBaseUrl).catch((error) => {
      setConfigurationError(String(error));
    });
  }, [draftBaseUrl, persistBaseUrl]);

  const handleApiKeyChange = useCallback((value: string) => {
    setDraftApiKey(value);
  }, []);

  const handleApiKeyBlur = useCallback(() => {
    void persistApiKey(draftApiKey).catch((error) => {
      setConfigurationError(String(error));
    });
  }, [draftApiKey, persistApiKey]);

  const savePendingChanges = useCallback(async () => {
    try {
      await persistBaseUrl(draftBaseUrl);
      await persistApiKey(draftApiKey);
      return true;
    } catch (error) {
      setConfigurationError(String(error));
      return false;
    }
  }, [draftApiKey, draftBaseUrl, persistApiKey, persistBaseUrl]);

  const handleModelChange = useCallback(
    (value: string) => {
      const trimmed = value.trim();
      if (trimmed !== model) {
        setConfigurationError("");
        void updatePostProcessModel(selectedProviderId, trimmed).catch(
          (error) => {
            setConfigurationError(String(error));
          },
        );
      }
    },
    [model, selectedProviderId, updatePostProcessModel],
  );

  const handleModelSelect = useCallback(
    (value: string) => {
      setConfigurationError("");
      void updatePostProcessModel(selectedProviderId, value.trim()).catch(
        (error) => {
          setConfigurationError(String(error));
        },
      );
    },
    [selectedProviderId, updatePostProcessModel],
  );

  const handleModelCreate = useCallback(
    (value: string) => {
      setConfigurationError("");
      void updatePostProcessModel(selectedProviderId, value).catch((error) => {
        setConfigurationError(String(error));
      });
    },
    [selectedProviderId, updatePostProcessModel],
  );

  const handleReasoningEffortChange = useCallback(
    (value: string) => {
      setConfigurationError("");
      void updatePostProcessReasoningEffort(selectedProviderId, value).catch(
        (error) => {
          setConfigurationError(String(error));
        },
      );
    },
    [selectedProviderId, updatePostProcessReasoningEffort],
  );

  const handleRefreshModels = useCallback(async () => {
    if (isAppleProvider) return [];
    setConfigurationError("");
    const saved = await savePendingChanges();
    if (!saved) return [];
    try {
      return await fetchPostProcessModels(selectedProviderId);
    } catch (error) {
      setConfigurationError(String(error));
      throw error;
    }
  }, [
    fetchPostProcessModels,
    isAppleProvider,
    savePendingChanges,
    selectedProviderId,
  ]);

  const availableModelsRaw = postProcessModelOptions[selectedProviderId] || [];

  const modelOptions = useMemo<ModelOption[]>(() => {
    const seen = new Set<string>();
    const options: ModelOption[] = [];

    const upsert = (
      value: string | null | undefined,
      label?: string,
      isDisabled?: boolean,
    ) => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) return;
      seen.add(trimmed);
      options.push({ value: trimmed, label: label ?? trimmed, isDisabled });
    };

    // Provider-curated suggestions are available without hitting the network.
    for (const candidate of selectedProvider?.model_suggestions ?? []) {
      upsert(candidate);
    }

    // Add available models from API
    for (const candidate of availableModelsRaw) {
      upsert(candidate);
    }

    for (const candidate of selectedProvider?.deprecated_model_suggestions ??
      []) {
      upsert(
        candidate,
        t("settings.postProcessing.api.model.deprecatedOption", {
          model: candidate,
        }),
      );
    }

    // Ensure current model is in the list
    upsert(model);

    return options;
  }, [
    availableModelsRaw,
    model,
    selectedProvider?.deprecated_model_suggestions,
    selectedProvider?.model_suggestions,
    t,
  ]);

  const reasoningOptions = useMemo<DropdownOption[]>(
    () => [
      {
        value: "disabled",
        label: t("settings.postProcessing.api.reasoning.options.disabled"),
      },
      {
        value: "high",
        label: t("settings.postProcessing.api.reasoning.options.high"),
      },
      {
        value: "max",
        label: t("settings.postProcessing.api.reasoning.options.max"),
      },
    ],
    [t],
  );

  const isBaseUrlUpdating = isUpdating(
    `post_process_base_url:${selectedProviderId}`,
  );
  const isApiKeyUpdating = isUpdating(
    `post_process_api_key:${selectedProviderId}`,
  );
  const isModelUpdating = isUpdating(
    `post_process_model:${selectedProviderId}`,
  );
  const isReasoningUpdating = isUpdating(
    `post_process_reasoning_effort:${selectedProviderId}`,
  );
  const isFetchingModels = isUpdating(
    `post_process_models_fetch:${selectedProviderId}`,
  );

  const isCustomProvider = selectedProvider?.id === "custom";

  // No automatic fetching - user must click refresh button

  return {
    providerOptions,
    selectedProviderId,
    selectedProvider,
    isCustomProvider,
    isAppleProvider,
    isDeepSeekProvider,
    appleIntelligenceUnavailable,
    configurationError,
    clearConfigurationError: () => setConfigurationError(""),
    baseUrl: draftBaseUrl,
    handleBaseUrlChange,
    handleBaseUrlBlur,
    isBaseUrlUpdating,
    apiKey: draftApiKey,
    handleApiKeyChange,
    handleApiKeyBlur,
    isApiKeyUpdating,
    model,
    handleModelChange,
    modelOptions,
    isModelUpdating,
    reasoningEffort,
    reasoningOptions,
    handleReasoningEffortChange,
    isReasoningUpdating,
    isFetchingModels,
    handleProviderSelect,
    handleModelSelect,
    handleModelCreate,
    handleRefreshModels,
    savePendingChanges,
  };
};
