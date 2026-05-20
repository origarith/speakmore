import React, { useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import {
  Copy,
  FlaskConical,
  Plus,
  RefreshCcw,
  Save,
  Trash2,
} from "lucide-react";
import { commands } from "@/bindings";
import type { PostProcessPreset, PostProcessPreviewResult } from "@/bindings";

import { Alert } from "../../ui/Alert";
import {
  Dropdown,
  SettingContainer,
  SettingsGroup,
  Textarea,
  ToggleSwitch,
} from "@/components/ui";
import { Button } from "../../ui/Button";
import { ResetButton } from "../../ui/ResetButton";
import { Input } from "../../ui/Input";

import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { BaseUrlField } from "../PostProcessingSettingsApi/BaseUrlField";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import {
  usePostProcessProviderState,
  type PostProcessProviderState,
} from "../PostProcessingSettingsApi/usePostProcessProviderState";
import { ShortcutInput } from "../ShortcutInput";
import { useSettings } from "../../../hooks/useSettings";

const APPLE_PROVIDER_ID = "apple_intelligence";
const CUSTOM_PROVIDER_ID = "custom";
const OUTPUT_PLACEHOLDER = "${output}";

const legacySystemTemplate = (promptTemplate: string) =>
  promptTemplate.split(OUTPUT_PLACEHOLDER).join("").trim();

const effectiveSystemTemplate = (preset: PostProcessPreset) =>
  (
    preset.system_template ?? legacySystemTemplate(preset.prompt_template)
  ).trim();

const effectiveUserTemplate = (preset: PostProcessPreset) =>
  (preset.user_template ?? OUTPUT_PLACEHOLDER).trim();

const PostProcessingOverview: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();

  const enabled = settings?.post_process_enabled ?? false;
  const providers = settings?.post_process_providers ?? [];
  const providerId = settings?.post_process_provider_id ?? "";
  const provider = providers.find((candidate) => candidate.id === providerId);
  const model = settings?.post_process_models?.[providerId]?.trim() ?? "";
  const apiKey = settings?.post_process_api_keys?.[providerId]?.trim() ?? "";
  const presets = settings?.post_process_presets ?? [];
  const selectedPresetId = settings?.post_process_selected_preset_id ?? "";
  const selectedPreset = presets.find(
    (preset) => preset.id === selectedPresetId,
  );
  const apiKeyNotRequired =
    providerId === APPLE_PROVIDER_ID || providerId === CUSTOM_PROVIDER_ID;
  const apiKeyConfigured = apiKeyNotRequired || apiKey.length > 0;
  const modelConfigured = model.length > 0;
  const ready =
    !!provider && !!selectedPreset && apiKeyConfigured && modelConfigured;
  const statusVariant = enabled ? (ready ? "success" : "warning") : "info";

  const summaryItems = [
    {
      label: t("settings.postProcessing.overview.items.status"),
      value: enabled
        ? t("settings.postProcessing.overview.values.enabled")
        : t("settings.postProcessing.overview.values.disabled"),
    },
    {
      label: t("settings.postProcessing.overview.items.provider"),
      value:
        provider?.label || t("settings.postProcessing.overview.values.missing"),
    },
    {
      label: t("settings.postProcessing.overview.items.model"),
      value: model || t("settings.postProcessing.overview.values.missing"),
    },
    {
      label: t("settings.postProcessing.overview.items.apiKey"),
      value: apiKeyNotRequired
        ? t("settings.postProcessing.overview.values.notRequired")
        : apiKeyConfigured
          ? t("settings.postProcessing.overview.values.configured")
          : t("settings.postProcessing.overview.values.missing"),
    },
    {
      label: t("settings.postProcessing.overview.items.preset"),
      value:
        selectedPreset?.name ||
        t("settings.postProcessing.overview.values.missing"),
    },
  ];

  return (
    <SettingsGroup title={t("settings.postProcessing.overview.title")}>
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("post_process_enabled", nextEnabled)
        }
        isUpdating={isUpdating("post_process_enabled")}
        label={t("settings.postProcessing.overview.toggle.label")}
        description={t("settings.postProcessing.overview.toggle.description")}
        descriptionMode="tooltip"
        grouped={true}
      />
      <div className="space-y-3 px-4 py-3">
        <Alert variant={statusVariant} contained>
          {enabled
            ? ready
              ? t("settings.postProcessing.overview.ready")
              : t("settings.postProcessing.overview.needsConfiguration")
            : t("settings.postProcessing.overview.disabled")}
        </Alert>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {summaryItems.map((item) => (
            <div
              key={item.label}
              className="min-w-0 rounded-md border border-frost-border bg-white/40 px-3 py-2 dark:bg-white/5"
            >
              <div className="text-[11px] font-semibold uppercase text-text/50">
                {item.label}
              </div>
              <div
                className="truncate text-sm font-semibold"
                title={item.value}
              >
                {item.value}
              </div>
            </div>
          ))}
        </div>
      </div>
    </SettingsGroup>
  );
};

type PostProcessingSettingsApiComponentProps = {
  state: PostProcessProviderState;
};

const PostProcessingSettingsApiComponent: React.FC<
  PostProcessingSettingsApiComponentProps
> = ({ state }) => {
  const { t } = useTranslation();
  const [apiMessage, setApiMessage] = useState("");
  const structuredOutputMode =
    state.selectedProvider?.structured_output_mode ??
    (state.selectedProvider?.supports_structured_output
      ? "open_ai_json_schema"
      : "none");
  const reasoningControl = state.selectedProvider?.reasoning_control ?? "none";
  const capabilitySummary = t(
    "settings.postProcessing.api.capabilities.summary",
    {
      structured: t(
        `settings.postProcessing.api.capabilities.structured.${structuredOutputMode}`,
      ),
      reasoning: t(
        `settings.postProcessing.api.capabilities.reasoning.${reasoningControl}`,
      ),
    },
  );
  const requiresApiKey = !state.isAppleProvider && !state.isCustomProvider;
  const missingApiKey = requiresApiKey && !state.apiKey.trim();
  const missingBaseUrl = state.isCustomProvider && !state.baseUrl.trim();
  const missingModel = !state.isAppleProvider && !state.model.trim();
  const configurationWarnings = [
    missingBaseUrl
      ? t("settings.postProcessing.api.status.missingBaseUrl")
      : null,
    missingApiKey
      ? t("settings.postProcessing.api.status.missingApiKey")
      : null,
    missingModel ? t("settings.postProcessing.api.status.missingModel") : null,
  ].filter(Boolean);

  const handleRefreshModels = async () => {
    state.clearConfigurationError();
    setApiMessage("");

    if (missingBaseUrl) {
      setApiMessage(t("settings.postProcessing.api.status.missingBaseUrl"));
      return;
    }
    if (missingApiKey) {
      setApiMessage(t("settings.postProcessing.api.status.missingApiKey"));
      return;
    }

    try {
      await state.handleRefreshModels();
    } catch {
      setApiMessage(t("settings.postProcessing.api.status.refreshFailed"));
    }
  };

  return (
    <>
      <SettingContainer
        title={t("settings.postProcessing.api.provider.title")}
        description={t("settings.postProcessing.api.provider.description")}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <div className="flex w-72 max-w-full min-w-0 items-center gap-2">
          <ProviderSelect
            options={state.providerOptions}
            value={state.selectedProviderId}
            onChange={state.handleProviderSelect}
          />
        </div>
      </SettingContainer>

      <Alert variant="info" contained>
        {state.isDeepSeekProvider
          ? t("settings.postProcessing.api.capabilities.deepseek")
          : capabilitySummary}
      </Alert>

      {state.isAppleProvider ? (
        state.appleIntelligenceUnavailable ? (
          <Alert variant="error" contained>
            {t("settings.postProcessing.api.appleIntelligence.unavailable")}
          </Alert>
        ) : null
      ) : (
        <>
          {state.selectedProvider?.id === "custom" && (
            <SettingContainer
              title={t("settings.postProcessing.api.baseUrl.title")}
              description={t("settings.postProcessing.api.baseUrl.description")}
              descriptionMode="tooltip"
              layout="horizontal"
              grouped={true}
            >
              <div className="flex w-96 max-w-full min-w-0 items-center gap-2">
                <BaseUrlField
                  value={state.baseUrl}
                  onChange={state.handleBaseUrlChange}
                  onBlur={state.handleBaseUrlBlur}
                  placeholder={t(
                    "settings.postProcessing.api.baseUrl.placeholder",
                  )}
                  disabled={state.isBaseUrlUpdating}
                />
              </div>
            </SettingContainer>
          )}

          <SettingContainer
            title={t("settings.postProcessing.api.apiKey.title")}
            description={t("settings.postProcessing.api.apiKey.description")}
            descriptionMode="tooltip"
            layout="horizontal"
            grouped={true}
          >
            <div className="flex w-80 max-w-full min-w-0 items-center gap-2">
              <ApiKeyField
                value={state.apiKey}
                onChange={state.handleApiKeyChange}
                onBlur={state.handleApiKeyBlur}
                placeholder={t(
                  "settings.postProcessing.api.apiKey.placeholder",
                )}
                disabled={state.isApiKeyUpdating}
              />
            </div>
          </SettingContainer>
        </>
      )}

      {!state.isAppleProvider && (
        <SettingContainer
          title={t("settings.postProcessing.api.model.title")}
          description={
            state.isCustomProvider
              ? t("settings.postProcessing.api.model.descriptionCustom")
              : t("settings.postProcessing.api.model.descriptionDefault")
          }
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <div className="flex min-w-0 items-center gap-2">
            <ModelSelect
              value={state.model}
              options={state.modelOptions}
              disabled={state.isModelUpdating}
              isLoading={state.isFetchingModels}
              placeholder={
                state.modelOptions.length > 0
                  ? t(
                      "settings.postProcessing.api.model.placeholderWithOptions",
                    )
                  : t("settings.postProcessing.api.model.placeholderNoOptions")
              }
              onSelect={state.handleModelSelect}
              onCreate={state.handleModelCreate}
              onBlur={() => {}}
              className="min-w-0 flex-1"
            />
            <ResetButton
              onClick={() => void handleRefreshModels()}
              disabled={state.isFetchingModels}
              ariaLabel={t("settings.postProcessing.api.model.refreshModels")}
              className="flex h-10 w-10 items-center justify-center"
            >
              <RefreshCcw
                className={`h-4 w-4 ${state.isFetchingModels ? "animate-spin" : ""}`}
              />
            </ResetButton>
          </div>
        </SettingContainer>
      )}

      {state.isDeepSeekProvider && (
        <SettingContainer
          title={t("settings.postProcessing.api.reasoning.title")}
          description={t("settings.postProcessing.api.reasoning.description")}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <div className="flex w-72 max-w-full min-w-0 items-center gap-2">
            <Dropdown
              selectedValue={state.reasoningEffort || "disabled"}
              options={state.reasoningOptions}
              onSelect={state.handleReasoningEffortChange}
              disabled={state.isReasoningUpdating}
              className="w-full min-w-0"
            />
          </div>
        </SettingContainer>
      )}

      {(state.configurationError || apiMessage) && (
        <Alert variant="error" contained>
          {state.configurationError || apiMessage}
        </Alert>
      )}

      {configurationWarnings.length > 0 &&
        !state.configurationError &&
        !apiMessage && (
          <Alert variant="warning" contained>
            {configurationWarnings.join(
              t("settings.postProcessing.api.status.separator"),
            )}
          </Alert>
        )}
    </>
  );
};

type PostProcessingSettingsPromptsComponentProps = {
  onBeforePreview: () => Promise<boolean>;
};

const PostProcessingSettingsPromptsComponent: React.FC<
  PostProcessingSettingsPromptsComponentProps
> = ({ onBeforePreview }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftDescription, setDraftDescription] = useState("");
  const [draftSystemTemplate, setDraftSystemTemplate] = useState("");
  const [draftUserTemplate, setDraftUserTemplate] =
    useState(OUTPUT_PLACEHOLDER);
  const [draftOutputKind, setDraftOutputKind] = useState("plain_text");
  const [formError, setFormError] = useState("");
  const [previewInput, setPreviewInput] = useState(
    t("settings.postProcessing.presets.preview.sampleText"),
  );
  const [previewResult, setPreviewResult] =
    useState<PostProcessPreviewResult | null>(null);
  const [isPreviewing, setIsPreviewing] = useState(false);

  const presets = (getSetting("post_process_presets") ||
    []) as PostProcessPreset[];
  const selectedPresetId = getSetting("post_process_selected_preset_id") || "";
  const selectedPreset =
    presets.find((preset) => preset.id === selectedPresetId) || null;
  const selectedIsBuiltin = !!selectedPreset?.is_builtin;

  const outputKindOptions = [
    {
      value: "plain_text",
      label: t("settings.postProcessing.presets.outputKinds.plainText"),
    },
    {
      value: "markdown",
      label: t("settings.postProcessing.presets.outputKinds.markdown"),
    },
  ];

  useEffect(() => {
    if (isCreating) return;

    if (selectedPreset) {
      setDraftName(selectedPreset.name);
      setDraftDescription(selectedPreset.description || "");
      setDraftSystemTemplate(effectiveSystemTemplate(selectedPreset));
      setDraftUserTemplate(effectiveUserTemplate(selectedPreset));
      setDraftOutputKind(selectedPreset.output_kind || "plain_text");
    } else {
      setDraftName("");
      setDraftDescription("");
      setDraftSystemTemplate("");
      setDraftUserTemplate(OUTPUT_PLACEHOLDER);
      setDraftOutputKind("plain_text");
    }
    setFormError("");
    setPreviewResult(null);
  }, [
    isCreating,
    selectedPresetId,
    selectedPreset?.name,
    selectedPreset?.description,
    selectedPreset?.prompt_template,
    selectedPreset?.system_template,
    selectedPreset?.user_template,
    selectedPreset?.output_kind,
  ]);

  const runCommand = async (
    command: () => Promise<
      { status: "ok"; data: unknown } | { status: "error"; error: string }
    >,
  ) => {
    const result = await command();
    if (result.status === "error") {
      setFormError(String(result.error));
      return false;
    }
    setFormError("");
    return true;
  };

  const handlePresetSelect = (presetId: string | null) => {
    if (!presetId) return;
    updateSetting("post_process_selected_preset_id", presetId);
    setIsCreating(false);
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setDraftName("");
    setDraftDescription("");
    setDraftSystemTemplate("");
    setDraftUserTemplate(OUTPUT_PLACEHOLDER);
    setDraftOutputKind("plain_text");
    setFormError("");
    setPreviewResult(null);
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    if (selectedPreset) {
      setDraftName(selectedPreset.name);
      setDraftDescription(selectedPreset.description || "");
      setDraftSystemTemplate(effectiveSystemTemplate(selectedPreset));
      setDraftUserTemplate(effectiveUserTemplate(selectedPreset));
      setDraftOutputKind(selectedPreset.output_kind || "plain_text");
    }
  };

  const handleCreatePreset = async () => {
    if (!draftName.trim() || !draftUserTemplate.trim()) return;

    const ok = await runCommand(() =>
      commands.addPostProcessPreset(
        draftName.trim(),
        draftDescription.trim(),
        draftSystemTemplate.trim(),
        draftUserTemplate.trim(),
        draftOutputKind,
      ),
    );
    if (ok) {
      await refreshSettings();
      setIsCreating(false);
    }
  };

  const handleUpdatePreset = async () => {
    if (
      !selectedPreset ||
      selectedIsBuiltin ||
      !draftName.trim() ||
      !draftUserTemplate.trim()
    ) {
      return false;
    }

    const ok = await runCommand(() =>
      commands.updatePostProcessPreset(
        selectedPreset.id,
        draftName.trim(),
        draftDescription.trim(),
        draftSystemTemplate.trim(),
        draftUserTemplate.trim(),
        draftOutputKind,
        true,
      ),
    );
    if (ok) await refreshSettings();
    return ok;
  };

  const handleDuplicatePreset = async () => {
    if (!selectedPreset) return;

    const ok = await runCommand(() =>
      commands.duplicatePostProcessPreset(selectedPreset.id),
    );
    if (ok) await refreshSettings();
    setIsCreating(false);
  };

  const handleDeletePreset = async () => {
    if (!selectedPreset || selectedIsBuiltin) return;

    const ok = await runCommand(() =>
      commands.deletePostProcessPreset(selectedPreset.id),
    );
    if (ok) await refreshSettings();
  };

  const handlePreview = async () => {
    if (!selectedPreset || !previewInput.trim()) return;

    setIsPreviewing(true);
    setPreviewResult(null);
    setFormError("");
    try {
      const providerSaved = await onBeforePreview();
      if (!providerSaved) return;

      if (isDirty) {
        const saved = await handleUpdatePreset();
        if (!saved) return;
      }

      const result = await commands.runPostProcessPresetPreview(
        selectedPreset.id,
        previewInput.trim(),
      );
      if (result.status === "ok") {
        setPreviewResult(result.data);
      } else {
        setPreviewResult({
          preset_id: selectedPreset.id,
          preset_version: selectedPreset.version || 1,
          provider_id: null,
          model: null,
          status: "failed",
          output_text: null,
          latency_ms: 0,
          error_summary: String(result.error),
        });
      }
    } catch {
      setPreviewResult({
        preset_id: selectedPreset.id,
        preset_version: selectedPreset.version || 1,
        provider_id: null,
        model: null,
        status: "failed",
        output_text: null,
        latency_ms: 0,
        error_summary: t("settings.postProcessing.presets.preview.failed"),
      });
    } finally {
      setIsPreviewing(false);
    }
  };

  const isDirty =
    !!selectedPreset &&
    !selectedIsBuiltin &&
    (draftName.trim() !== selectedPreset.name ||
      draftDescription.trim() !== (selectedPreset.description || "") ||
      draftSystemTemplate.trim() !== effectiveSystemTemplate(selectedPreset) ||
      draftUserTemplate.trim() !== effectiveUserTemplate(selectedPreset) ||
      draftOutputKind !== (selectedPreset.output_kind || "plain_text"));

  return (
    <SettingContainer
      title={t("settings.postProcessing.presets.selectedPreset.title")}
      description={t(
        "settings.postProcessing.presets.selectedPreset.description",
      )}
      descriptionMode="tooltip"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-4">
        <div className="flex min-w-0 flex-col gap-2 sm:flex-row">
          <Dropdown
            selectedValue={isCreating ? null : selectedPresetId || null}
            options={presets.map((preset) => ({
              value: preset.id,
              label: preset.is_builtin
                ? t("settings.postProcessing.presets.builtinLabel", {
                    name: preset.name,
                  })
                : preset.name,
            }))}
            onSelect={(value) => handlePresetSelect(value)}
            placeholder={
              presets.length === 0
                ? t("settings.postProcessing.presets.noPresets")
                : t("settings.postProcessing.presets.selectPreset")
            }
            disabled={
              isUpdating("post_process_selected_preset_id") || isCreating
            }
            className="min-w-0 flex-1"
          />
          <Button
            onClick={handleStartCreate}
            variant="primary"
            size="md"
            disabled={isCreating}
            className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap"
          >
            <Plus className="h-4 w-4" />
            <span>{t("settings.postProcessing.presets.createNew")}</span>
          </Button>
        </div>

        {formError && (
          <Alert variant="error" contained>
            {formError}
          </Alert>
        )}

        {!isCreating && selectedPreset && selectedIsBuiltin && (
          <div className="space-y-3">
            <Alert variant="info" contained>
              {t("settings.postProcessing.presets.builtinNotice")}
            </Alert>
            <div className="grid grid-cols-1 gap-3 rounded-md border border-frost-border bg-white/40 p-3 dark:bg-white/5 sm:grid-cols-2">
              <div className="min-w-0">
                <div className="text-xs font-semibold text-text/50">
                  {t("settings.postProcessing.presets.nameLabel")}
                </div>
                <div
                  className="truncate text-sm font-semibold"
                  title={selectedPreset.name}
                >
                  {selectedPreset.name}
                </div>
              </div>
              <div className="min-w-0">
                <div className="text-xs font-semibold text-text/50">
                  {t("settings.postProcessing.presets.outputKindLabel")}
                </div>
                <div className="truncate text-sm font-semibold">
                  {draftOutputKind === "markdown"
                    ? t("settings.postProcessing.presets.outputKinds.markdown")
                    : t(
                        "settings.postProcessing.presets.outputKinds.plainText",
                      )}
                </div>
              </div>
              {selectedPreset.description && (
                <div className="min-w-0 sm:col-span-2">
                  <div className="text-xs font-semibold text-text/50">
                    {t("settings.postProcessing.presets.descriptionLabel")}
                  </div>
                  <div className="text-sm font-semibold">
                    {selectedPreset.description}
                  </div>
                </div>
              )}
            </div>
            <div className="space-y-3">
              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.systemTemplateLabel")}
                </label>
                <Textarea
                  value={draftSystemTemplate}
                  readOnly
                  variant="compact"
                  className="min-h-[120px]"
                />
              </div>
              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.userTemplateLabel")}
                </label>
                <Textarea
                  value={draftUserTemplate}
                  readOnly
                  variant="compact"
                  className="min-h-[80px]"
                />
              </div>
            </div>
            <Button
              onClick={handleDuplicatePreset}
              variant="secondary"
              size="md"
              disabled={!selectedPreset}
              className="inline-flex items-center gap-2 whitespace-nowrap"
            >
              <Copy className="h-4 w-4" />
              <span>
                {t("settings.postProcessing.presets.duplicatePreset")}
              </span>
            </Button>
          </div>
        )}

        {(isCreating || (selectedPreset && !selectedIsBuiltin)) && (
          <div className="space-y-3">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.nameLabel")}
                </label>
                <Input
                  type="text"
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  placeholder={t(
                    "settings.postProcessing.presets.namePlaceholder",
                  )}
                  variant="compact"
                  disabled={!isCreating && selectedIsBuiltin}
                />
              </div>

              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.outputKindLabel")}
                </label>
                <Dropdown
                  selectedValue={draftOutputKind}
                  options={outputKindOptions}
                  onSelect={(value) => setDraftOutputKind(value)}
                  disabled={!isCreating && selectedIsBuiltin}
                  className="w-full min-w-0"
                />
              </div>
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.presets.descriptionLabel")}
              </label>
              <Input
                type="text"
                value={draftDescription}
                onChange={(e) => setDraftDescription(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.presets.descriptionPlaceholder",
                )}
                variant="compact"
                disabled={!isCreating && selectedIsBuiltin}
              />
            </div>

            <div className="space-y-3">
              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.systemTemplateLabel")}
                </label>
                <Textarea
                  value={draftSystemTemplate}
                  onChange={(e) => setDraftSystemTemplate(e.target.value)}
                  placeholder={t(
                    "settings.postProcessing.presets.systemTemplatePlaceholder",
                  )}
                  disabled={!isCreating && selectedIsBuiltin}
                  className="min-h-[120px]"
                />
              </div>

              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.userTemplateLabel")}
                </label>
                <Textarea
                  value={draftUserTemplate}
                  onChange={(e) => setDraftUserTemplate(e.target.value)}
                  placeholder={t(
                    "settings.postProcessing.presets.userTemplatePlaceholder",
                  )}
                  disabled={!isCreating && selectedIsBuiltin}
                  className="min-h-[96px]"
                />
              </div>

              <Alert variant="info" contained className="py-3">
                <Trans
                  i18nKey="settings.postProcessing.presets.templateTip"
                  components={{ code: <code /> }}
                />
              </Alert>
            </div>

            <div className="flex flex-wrap gap-2 pt-1">
              {isCreating ? (
                <>
                  <Button
                    onClick={handleCreatePreset}
                    variant="primary"
                    size="md"
                    disabled={!draftName.trim() || !draftUserTemplate.trim()}
                    className="inline-flex items-center gap-2 whitespace-nowrap"
                  >
                    <Plus className="h-4 w-4" />
                    <span>
                      {t("settings.postProcessing.presets.createPreset")}
                    </span>
                  </Button>
                  <Button
                    onClick={handleCancelCreate}
                    variant="secondary"
                    size="md"
                  >
                    {t("settings.postProcessing.presets.cancel")}
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    onClick={handleUpdatePreset}
                    variant="primary"
                    size="md"
                    disabled={
                      selectedIsBuiltin ||
                      !draftName.trim() ||
                      !draftUserTemplate.trim() ||
                      !isDirty
                    }
                    className="inline-flex items-center gap-2 whitespace-nowrap"
                  >
                    <Save className="h-4 w-4" />
                    <span>
                      {t("settings.postProcessing.presets.savePreset")}
                    </span>
                  </Button>
                  <Button
                    onClick={handleDuplicatePreset}
                    variant="secondary"
                    size="md"
                    disabled={!selectedPreset}
                    className="inline-flex items-center gap-2 whitespace-nowrap"
                  >
                    <Copy className="h-4 w-4" />
                    <span>
                      {t("settings.postProcessing.presets.duplicatePreset")}
                    </span>
                  </Button>
                  <Button
                    onClick={handleDeletePreset}
                    variant="danger-ghost"
                    size="md"
                    disabled={!selectedPreset || selectedIsBuiltin}
                    className="inline-flex items-center gap-2 whitespace-nowrap"
                  >
                    <Trash2 className="h-4 w-4" />
                    <span>
                      {t("settings.postProcessing.presets.deletePreset")}
                    </span>
                  </Button>
                </>
              )}
            </div>
          </div>
        )}

        {!isCreating && !selectedPreset && (
          <div className="rounded-md border border-frost-border bg-white/40 p-3 dark:bg-white/5">
            <p className="text-sm text-text/60">
              {presets.length > 0
                ? t("settings.postProcessing.presets.selectToEdit")
                : t("settings.postProcessing.presets.createFirst")}
            </p>
          </div>
        )}

        {!isCreating && selectedPreset && (
          <div className="space-y-3 border-t border-frost-border pt-4">
            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.presets.preview.inputLabel")}
              </label>
              <Textarea
                value={previewInput}
                onChange={(e) => setPreviewInput(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.presets.preview.inputPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div className="flex items-center gap-2">
              <Button
                onClick={handlePreview}
                variant="secondary"
                size="md"
                disabled={!previewInput.trim() || isPreviewing}
                className="inline-flex items-center gap-2"
              >
                <FlaskConical className="h-4 w-4" />
                <span>
                  {isPreviewing
                    ? t("settings.postProcessing.presets.preview.running")
                    : t("settings.postProcessing.presets.preview.run")}
                </span>
              </Button>
              {previewResult && (
                <span className="text-xs text-text/60">
                  {t("settings.postProcessing.presets.preview.latency", {
                    ms: previewResult.latency_ms,
                  })}
                </span>
              )}
            </div>

            {previewResult && (
              <div className="text-xs text-text/60">
                {t("settings.postProcessing.presets.preview.metadata", {
                  provider:
                    previewResult.provider_id ||
                    t("settings.postProcessing.presets.preview.unknown"),
                  model:
                    previewResult.model ||
                    t("settings.postProcessing.presets.preview.unknown"),
                })}
              </div>
            )}

            {previewResult?.status === "success" && (
              <div className="space-y-2 flex flex-col">
                <label className="text-sm font-semibold">
                  {t("settings.postProcessing.presets.preview.outputLabel")}
                </label>
                <Textarea
                  value={previewResult.output_text || ""}
                  readOnly
                  variant="compact"
                />
              </div>
            )}

            {previewResult?.status === "failed" && (
              <Alert variant="warning" contained>
                {previewResult.error_summary ||
                  t("settings.postProcessing.presets.preview.failed")}
              </Alert>
            )}
          </div>
        )}
      </div>
    </SettingContainer>
  );
};

export const PostProcessingSettingsApi = React.memo(
  PostProcessingSettingsApiComponent,
);
PostProcessingSettingsApi.displayName = "PostProcessingSettingsApi";

export const PostProcessingSettingsPrompts = React.memo(
  PostProcessingSettingsPromptsComponent,
);
PostProcessingSettingsPrompts.displayName = "PostProcessingSettingsPrompts";

export const PostProcessingSettings: React.FC = () => {
  const { t } = useTranslation();
  const providerState = usePostProcessProviderState();

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <PostProcessingOverview />

      <SettingsGroup title={t("settings.postProcessing.hotkey.title")}>
        <ShortcutInput
          shortcutId="transcribe_with_post_process"
          descriptionMode="tooltip"
          grouped={true}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.postProcessing.api.title")}>
        <PostProcessingSettingsApi state={providerState} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.postProcessing.presets.title")}>
        <PostProcessingSettingsPrompts
          onBeforePreview={providerState.savePendingChanges}
        />
      </SettingsGroup>
    </div>
  );
};
