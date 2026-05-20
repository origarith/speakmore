import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, RotateCcw, X } from "lucide-react";
import type {
  ModelInfo,
  Qwen3AsrFamilySettings,
  WhisperFamilySettings,
} from "@/bindings";
import { commands } from "@/bindings";
import { LANGUAGES } from "@/lib/constants/languages";
import { useSettings } from "@/hooks/useSettings";
import {
  Alert,
  Button,
  Input,
  Select,
  Textarea,
  ToggleSwitch,
} from "@/components/ui";
import { PathDisplay } from "@/components/ui/PathDisplay";

type AsrFamily = "whisper" | "qwen3_asr";

interface ModelAdvancedSettingsDialogProps {
  model: ModelInfo;
  onClose: () => void;
}

interface FieldRowProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

const DEFAULT_WHISPER_SETTINGS: WhisperFamilySettings = {
  language: "auto",
  translate_to_english: false,
  custom_vocabulary: [],
};

const DEFAULT_QWEN_SETTINGS: Qwen3AsrFamilySettings = {
  language: "auto",
  custom_vocabulary: [],
  max_new_tokens: 384,
  max_total_len: 1024,
};

const CONTROL_OR_PROMPT_BREAKING_CHARS = /[\u0000-\u001f\u007f<>`]/;

const familyForModel = (model: ModelInfo): AsrFamily =>
  model.engine_type === "Qwen3Asr" ? "qwen3_asr" : "whisper";

const vocabularyToText = (words: string[]) => words.join("\n");

const settingsPathFromAppDir = (appDir: string): string => {
  const separator = appDir.includes("\\") ? "\\" : "/";
  const trimmed = appDir.replace(/[\\/]+$/, "");
  return `${trimmed}${separator}settings_store.json`;
};

const FieldRow: React.FC<FieldRowProps> = ({
  label,
  description,
  children,
}) => (
  <div className="grid gap-2 sm:grid-cols-[160px_minmax(0,1fr)] sm:gap-5">
    <div className="space-y-1 sm:pt-2">
      <label className="block text-sm font-semibold text-text">{label}</label>
      {description && (
        <p className="text-xs leading-relaxed text-text/50">{description}</p>
      )}
    </div>
    <div className="min-w-0">{children}</div>
  </div>
);

export const ModelAdvancedSettingsDialog: React.FC<
  ModelAdvancedSettingsDialogProps
> = ({ model, onClose }) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const family = familyForModel(model);

  const whisperSettings =
    settings?.asr_family_settings?.whisper ?? DEFAULT_WHISPER_SETTINGS;
  const qwenSettings =
    settings?.asr_family_settings?.qwen3_asr ?? DEFAULT_QWEN_SETTINGS;

  const [language, setLanguage] = useState("auto");
  const [translateToEnglish, setTranslateToEnglish] = useState(false);
  const [customVocabularyText, setCustomVocabularyText] = useState("");
  const [maxNewTokens, setMaxNewTokens] = useState(
    DEFAULT_QWEN_SETTINGS.max_new_tokens,
  );
  const [maxTotalLen, setMaxTotalLen] = useState(1024);
  const [settingsPath, setSettingsPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [showExpertSettings, setShowExpertSettings] = useState(false);

  useEffect(() => {
    if (family === "qwen3_asr") {
      setLanguage(qwenSettings.language || "auto");
      setCustomVocabularyText(vocabularyToText(qwenSettings.custom_vocabulary));
      setMaxNewTokens(
        qwenSettings.max_new_tokens || DEFAULT_QWEN_SETTINGS.max_new_tokens,
      );
      setMaxTotalLen(qwenSettings.max_total_len || 1024);
      setTranslateToEnglish(false);
    } else {
      setLanguage(whisperSettings.language || "auto");
      setCustomVocabularyText(
        vocabularyToText(whisperSettings.custom_vocabulary),
      );
      setTranslateToEnglish(whisperSettings.translate_to_english);
      setMaxNewTokens(DEFAULT_QWEN_SETTINGS.max_new_tokens);
      setMaxTotalLen(512);
    }
    setShowExpertSettings(false);
    setError(null);
  }, [family, qwenSettings, whisperSettings]);

  useEffect(() => {
    void commands.getAppDirPath().then((result) => {
      if (result.status === "ok") {
        setSettingsPath(settingsPathFromAppDir(result.data));
      }
    });
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const languageOptions = useMemo(() => {
    const supportedLanguages = new Set(model.supported_languages);
    return LANGUAGES.filter((lang) => {
      if (lang.value === "auto" || supportedLanguages.size === 0) {
        return true;
      }
      if (supportedLanguages.has(lang.value)) {
        return true;
      }
      return (
        supportedLanguages.has("zh") &&
        (lang.value === "zh-Hans" || lang.value === "zh-Hant")
      );
    }).map((lang) => ({ value: lang.value, label: lang.label }));
  }, [model.supported_languages]);

  const vocabularyLabel =
    family === "qwen3_asr"
      ? t("settings.models.advanced.qwenHotwordsLabel")
      : t("settings.models.advanced.customVocabularyLabel");
  const vocabularyPlaceholder =
    family === "qwen3_asr"
      ? t("settings.models.advanced.qwenHotwordsPlaceholder")
      : t("settings.models.advanced.customVocabularyPlaceholder");
  const vocabularyDescription =
    family === "qwen3_asr"
      ? t("settings.models.advanced.qwenHotwordsHint")
      : t("settings.models.advanced.whisperCustomVocabularyHint");

  const validateVocabulary = (): string[] | null => {
    const entries = customVocabularyText
      .split(/\r?\n/)
      .map((entry) => entry.trim())
      .filter(Boolean);
    const uniqueEntries = Array.from(new Set(entries));

    if (uniqueEntries.length > 200) {
      setError(t("settings.models.advanced.validation.tooManyVocabulary"));
      return null;
    }

    for (const entry of uniqueEntries) {
      if (entry.length > 80) {
        setError(
          t("settings.models.advanced.validation.vocabularyTooLong", {
            entry,
          }),
        );
        return null;
      }
      if (CONTROL_OR_PROMPT_BREAKING_CHARS.test(entry)) {
        setError(
          t("settings.models.advanced.validation.unsupportedVocabularyChars", {
            entry,
          }),
        );
        return null;
      }
    }

    return uniqueEntries;
  };

  const handleSave = async () => {
    const customVocabulary = validateVocabulary();
    if (!customVocabulary) return;

    setIsSaving(true);
    setError(null);
    try {
      const result =
        family === "qwen3_asr"
          ? await commands.updateQwen3AsrFamilySettings({
              language,
              custom_vocabulary: customVocabulary,
              max_new_tokens: maxNewTokens,
              max_total_len: Math.max(maxTotalLen, maxNewTokens),
            })
          : await commands.updateWhisperFamilySettings({
              language,
              translate_to_english: translateToEnglish,
              custom_vocabulary: customVocabulary,
            });

      if (result.status === "error") {
        throw new Error(result.error);
      }

      await refreshSettings();
      onClose();
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t("settings.models.advanced.saveFailed"),
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleReset = async () => {
    setIsSaving(true);
    setError(null);
    try {
      const result = await commands.resetAsrFamilySettings(family);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      if (family === "qwen3_asr") {
        setLanguage(DEFAULT_QWEN_SETTINGS.language);
        setCustomVocabularyText("");
        setMaxNewTokens(DEFAULT_QWEN_SETTINGS.max_new_tokens);
        setMaxTotalLen(DEFAULT_QWEN_SETTINGS.max_total_len);
      } else {
        setLanguage(DEFAULT_WHISPER_SETTINGS.language);
        setTranslateToEnglish(DEFAULT_WHISPER_SETTINGS.translate_to_english);
        setCustomVocabularyText("");
      }
      await refreshSettings();
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : t("settings.models.advanced.resetFailed"),
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleOpenSettingsFolder = async () => {
    const result = await commands.openAppDataDir();
    if (result.status === "error") {
      setError(result.error);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 px-4 py-6 backdrop-blur-sm"
      role="presentation"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-advanced-settings-title"
        className="warm-panel flex max-h-[calc(100vh-3rem)] w-full max-w-[680px] flex-col overflow-hidden rounded-xl border-frost-border shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4 border-b border-frost-border px-5 py-4 sm:px-6">
          <div className="min-w-0">
            <h2
              id="model-advanced-settings-title"
              className="text-lg font-semibold leading-tight text-text"
            >
              {t("settings.models.advanced.title", { model: model.name })}
            </h2>
            <p className="mt-1 text-sm leading-relaxed text-text/60">
              {t("settings.models.advanced.familyScope", {
                family:
                  family === "qwen3_asr"
                    ? t("settings.models.advanced.qwenFamilyName")
                    : t("settings.models.advanced.whisperFamilyName"),
              })}
            </p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            aria-label={t("common.close")}
            className="h-8 w-8 shrink-0 p-0"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 sm:px-6">
          <div className="space-y-5">
            {error && (
              <Alert variant="error" className="px-3 py-3">
                {error}
              </Alert>
            )}

            {family === "qwen3_asr" && (
              <Alert variant="info" className="px-3 py-3">
                {t("settings.models.advanced.qwenTranslationUnavailable")}
              </Alert>
            )}

            <FieldRow
              label={t("settings.models.advanced.languageLabel")}
              description={t("settings.models.advanced.languageDescription")}
            >
              <Select
                value={language}
                options={languageOptions}
                isClearable={false}
                onChange={(value) => setLanguage(value ?? "auto")}
                placeholder={t("settings.models.advanced.languagePlaceholder")}
                className="w-full"
              />
            </FieldRow>

            <FieldRow
              label={vocabularyLabel}
              description={vocabularyDescription}
            >
              <Textarea
                value={customVocabularyText}
                onChange={(event) =>
                  setCustomVocabularyText(event.target.value)
                }
                placeholder={vocabularyPlaceholder}
                className="min-h-[96px] w-full resize-y"
              />
            </FieldRow>

            {family === "qwen3_asr" && (
              <Alert variant="info" className="px-3 py-3">
                {t("settings.models.advanced.qwenPromptLimitation")}
              </Alert>
            )}

            {family === "whisper" ? (
              <ToggleSwitch
                checked={translateToEnglish}
                onChange={setTranslateToEnglish}
                label={t("settings.models.advanced.translateToEnglishLabel")}
                description={t(
                  "settings.models.advanced.translateToEnglishDescription",
                )}
                descriptionMode="inline"
              />
            ) : (
              <div className="overflow-hidden rounded-lg border border-frost-border">
                <button
                  type="button"
                  className="flex w-full items-center justify-between gap-3 px-3 py-3 text-left transition hover:bg-black/[0.03] dark:hover:bg-white/[0.04]"
                  onClick={() => setShowExpertSettings((current) => !current)}
                  aria-expanded={showExpertSettings}
                >
                  <span className="min-w-0">
                    <span className="block text-sm font-semibold text-text">
                      {t("settings.models.advanced.expertSettingsLabel")}
                    </span>
                    <span className="mt-0.5 block text-xs leading-relaxed text-text/55">
                      {t("settings.models.advanced.expertSettingsSummary")}
                    </span>
                  </span>
                  <ChevronDown
                    className={`h-4 w-4 shrink-0 text-text/50 transition-transform ${
                      showExpertSettings ? "rotate-180" : ""
                    }`}
                  />
                </button>

                {showExpertSettings && (
                  <div className="grid gap-3 border-t border-frost-border bg-black/[0.015] p-3 dark:bg-white/[0.03] sm:grid-cols-2">
                    <div>
                      <label className="block text-sm font-semibold text-text">
                        {t("settings.models.advanced.maxNewTokensLabel")}
                      </label>
                      <p className="mt-1 text-xs leading-relaxed text-text/50">
                        {t("settings.models.advanced.maxNewTokensDescription")}
                      </p>
                      <Input
                        type="number"
                        min={16}
                        max={512}
                        value={maxNewTokens}
                        onChange={(event) =>
                          setMaxNewTokens(Number(event.target.value))
                        }
                        className="mt-3 h-10 w-28 tabular-nums"
                      />
                    </div>
                    <div>
                      <label className="block text-sm font-semibold text-text">
                        {t("settings.models.advanced.maxTotalLenLabel")}
                      </label>
                      <p className="mt-1 text-xs leading-relaxed text-text/50">
                        {t("settings.models.advanced.maxTotalLenDescription")}
                      </p>
                      <Input
                        type="number"
                        min={128}
                        max={2048}
                        value={maxTotalLen}
                        onChange={(event) =>
                          setMaxTotalLen(Number(event.target.value))
                        }
                        className="mt-3 h-10 w-28 tabular-nums"
                      />
                    </div>
                  </div>
                )}
              </div>
            )}

            <FieldRow label={t("settings.models.advanced.settingsFileLabel")}>
              <PathDisplay
                path={
                  settingsPath ||
                  t("settings.models.advanced.settingsPathLoading")
                }
                onOpen={handleOpenSettingsFolder}
                disabled={!settingsPath}
                wrap={false}
              />
            </FieldRow>
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-frost-border bg-[var(--color-frost-surface)] px-5 py-4 sm:px-6">
          <Button
            variant="ghost"
            size="sm"
            onClick={handleReset}
            disabled={isSaving}
            className="flex items-center gap-1.5 text-text/65 hover:text-logo-primary"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            <span>{t("common.reset")}</span>
          </Button>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={onClose}
              disabled={isSaving}
            >
              {t("common.cancel")}
            </Button>
            <Button size="sm" onClick={handleSave} disabled={isSaving}>
              {isSaving ? t("common.saving") : t("common.save")}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
