import React from "react";
import { useTranslation } from "react-i18next";
import {
  BadgeCheck,
  Check,
  Cpu,
  Download,
  Globe,
  Gauge,
  HardDriveDownload,
  Languages,
  Loader2,
  PackageCheck,
  Settings2,
  Sparkles,
  Trash2,
} from "lucide-react";
import type { ModelInfo } from "@/bindings";
import { formatModelSize } from "../../lib/utils/format";
import { getModelSeriesDefinition } from "../../lib/utils/modelSeries";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../lib/utils/modelTranslation";
import { LANGUAGES } from "../../lib/constants/languages";
import Badge from "../ui/Badge";
import { Button } from "../ui/Button";

// Get display text for model's language support
const getLanguageDisplayText = (
  supportedLanguages: string[],
  t: (key: string, options?: Record<string, unknown>) => string,
): string => {
  if (supportedLanguages.length === 1) {
    const langCode = supportedLanguages[0];
    const langName =
      LANGUAGES.find((l) => l.value === langCode)?.label || langCode;
    return t("modelSelector.capabilities.languageOnly", { language: langName });
  }
  return t("modelSelector.capabilities.multiLanguage");
};

const getSourceIcon = (source: ModelInfo["source"]) => {
  if (source === "SpeakMore") return Sparkles;
  if (source === "Custom") return Cpu;
  return PackageCheck;
};

export type ModelCardStatus =
  | "downloadable"
  | "downloading"
  | "verifying"
  | "extracting"
  | "switching"
  | "active"
  | "available";

interface ModelCardProps {
  model: ModelInfo;
  variant?: "default" | "featured";
  status?: ModelCardStatus;
  disabled?: boolean;
  className?: string;
  onSelect: (modelId: string) => void;
  onDownload?: (modelId: string) => void;
  onDelete?: (modelId: string) => void;
  onCancel?: (modelId: string) => void;
  onAdvancedSettings?: (modelId: string) => void;
  downloadProgress?: number;
  downloadSpeed?: number; // MB/s
  showRecommended?: boolean;
}

const ModelCard: React.FC<ModelCardProps> = ({
  model,
  variant = "default",
  status = "downloadable",
  disabled = false,
  className = "",
  onSelect,
  onDownload,
  onDelete,
  onCancel,
  onAdvancedSettings,
  downloadProgress,
  downloadSpeed,
  showRecommended = true,
}) => {
  const { t } = useTranslation();
  const isFeatured = variant === "featured";
  const isClickable =
    status === "available" || status === "active" || status === "downloadable";
  const supportsAdvancedSettings =
    model.engine_type === "Whisper" || model.engine_type === "Qwen3Asr";
  const isBusy =
    status === "downloading" ||
    status === "verifying" ||
    status === "extracting" ||
    status === "switching";
  const series = getModelSeriesDefinition(model);
  const SourceIcon = getSourceIcon(series.source);

  // Get translated model name and description
  const displayName = getTranslatedModelName(model, t);
  const displayDescription = getTranslatedModelDescription(model, t);
  const modelSize = formatModelSize(Number(model.size_mb));
  const hasScores = model.accuracy_score > 0 || model.speed_score > 0;

  const baseClasses =
    "warm-panel flex min-h-[118px] flex-col overflow-hidden p-0 text-left transition-[background-color,border-color,box-shadow,transform] duration-200";

  const getVariantClasses = () => {
    if (status === "active") {
      return "border-logo-primary/40 bg-logo-primary/10 shadow-[0_14px_30px_rgba(47,143,131,0.16)]";
    }
    if (isFeatured) {
      return "border-logo-primary/25 bg-logo-primary/5";
    }
    return "border-frost-border";
  };

  const getInteractiveClasses = () => {
    if (!isClickable || isBusy) return "";
    if (disabled) return "opacity-50 cursor-not-allowed";
    return "cursor-pointer hover:border-logo-primary/40 hover:bg-logo-primary/5 hover:shadow-[0_16px_34px_rgba(92,66,41,0.14)] hover:-translate-y-0.5 active:translate-y-0 group";
  };

  const handleClick = () => {
    if (!isClickable || disabled || isBusy) return;
    if (status === "downloadable" && onDownload) {
      onDownload(model.id);
    } else {
      onSelect(model.id);
    }
  };

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete?.(model.id);
  };

  const handleAdvancedSettings = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onAdvancedSettings?.(model.id);
  };

  const renderScore = (label: string, value: number) => (
    <span
      className="inline-flex h-7 items-center gap-1.5 rounded-md border border-frost-border bg-white/25 px-2 text-[11px] font-medium text-text/55 dark:bg-white/5"
      title={`${label}: ${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`}
    >
      <span className="max-w-[4.5rem] truncate">{label}</span>
      <span className="h-1.5 w-9 overflow-hidden rounded-full bg-mid-gray/15">
        <div
          className="h-full rounded-full bg-logo-primary transition-[width] duration-300"
          style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }}
        />
      </span>
    </span>
  );

  return (
    <div
      onClick={handleClick}
      onKeyDown={(e) => {
        if ((e.key === "Enter" || e.key === " ") && isClickable) {
          e.preventDefault();
          handleClick();
        }
      }}
      role={isClickable ? "button" : undefined}
      tabIndex={isClickable ? 0 : undefined}
      aria-disabled={disabled || isBusy ? true : undefined}
      aria-label={isClickable ? displayName : undefined}
      className={[
        baseClasses,
        getVariantClasses(),
        getInteractiveClasses(),
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="flex flex-1 flex-col p-3">
        <div className="flex min-w-0 gap-2.5">
          <div
            className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md border ${
              status === "active"
                ? "border-logo-primary/25 bg-logo-primary/10 text-logo-primary"
                : "border-frost-border bg-white/30 text-text/50 dark:bg-white/10"
            }`}
          >
            {status === "active" ? (
              <BadgeCheck className="h-[18px] w-[18px]" />
            ) : status === "downloadable" ? (
              <HardDriveDownload className="h-[18px] w-[18px]" />
            ) : isBusy ? (
              <Loader2 className="h-[18px] w-[18px] animate-spin" />
            ) : (
              <SourceIcon className="h-[18px] w-[18px]" />
            )}
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-1.5">
              <h3
                className={`min-w-0 text-sm font-semibold leading-snug text-text ${isClickable ? "group-hover:text-logo-primary" : ""} transition-colors`}
              >
                {displayName}
              </h3>
              <Badge
                variant={
                  series.source === "SpeakMore" ? "primary" : "secondary"
                }
              >
                <SourceIcon className="mr-1 h-3 w-3" />
                {t(series.badgeKey)}
              </Badge>
              {showRecommended && model.is_recommended && (
                <Badge variant="success">{t("onboarding.recommended")}</Badge>
              )}
              {status === "active" && (
                <Badge variant="primary">
                  <Check className="mr-1 h-3 w-3" />
                  {t("modelSelector.active")}
                </Badge>
              )}
              {status === "available" && (
                <Badge variant="secondary">
                  {t("settings.models.downloaded")}
                </Badge>
              )}
              {status === "switching" && (
                <Badge variant="secondary">
                  <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                  {t("modelSelector.switching")}
                </Badge>
              )}
            </div>
            <p
              className="mt-1 text-xs leading-relaxed text-text/60"
              title={displayDescription}
              style={{
                display: "-webkit-box",
                overflow: "hidden",
                WebkitBoxOrient: "vertical",
                WebkitLineClamp: 2,
              }}
            >
              {displayDescription}
            </p>
          </div>
        </div>

        <div className="mt-3 flex min-h-8 flex-wrap items-center gap-x-3 gap-y-2 border-t border-frost-border pt-2">
          {model.supported_languages.length > 0 && (
            <div
              className="flex items-center gap-1.5 text-[11px] font-medium text-text/50"
              title={
                model.supported_languages.length === 1
                  ? t("modelSelector.capabilities.singleLanguage")
                  : t("modelSelector.capabilities.languageSelection")
              }
            >
              <Globe className="h-3.5 w-3.5" />
              <span>
                {getLanguageDisplayText(model.supported_languages, t)}
              </span>
            </div>
          )}
          {model.supports_translation && (
            <div
              className="flex items-center gap-1.5 text-[11px] font-medium text-text/50"
              title={t("modelSelector.capabilities.translation")}
            >
              <Languages className="h-3.5 w-3.5" />
              <span>{t("modelSelector.capabilities.translate")}</span>
            </div>
          )}
          {hasScores && (
            <div className="flex min-w-0 flex-wrap items-center gap-1.5">
              {status !== "downloadable" && (
                <span className="inline-flex h-7 items-center gap-1.5 rounded-md border border-frost-border bg-white/25 px-2 text-[11px] font-semibold text-text/50 dark:bg-white/5">
                  <Gauge className="h-3.5 w-3.5" />
                  <span>{modelSize}</span>
                </span>
              )}
              {renderScore(
                t("onboarding.modelCard.accuracy"),
                model.accuracy_score,
              )}
              {renderScore(t("onboarding.modelCard.speed"), model.speed_score)}
            </div>
          )}
          <div className="ms-auto flex min-h-8 items-center gap-1.5">
            {status === "downloadable" && (
              <span className="flex h-7 items-center gap-1.5 rounded-md border border-logo-primary/15 bg-logo-primary/10 px-2 text-[11px] font-semibold text-logo-primary">
                <Download className="h-3.5 w-3.5" />
                <span>{modelSize}</span>
              </span>
            )}
            {onAdvancedSettings && supportsAdvancedSettings && (
              <Button
                variant="ghost"
                size="sm"
                onClick={handleAdvancedSettings}
                title={t("modelSelector.advancedSettings", {
                  modelName: displayName,
                })}
                aria-label={t("modelSelector.advancedSettings", {
                  modelName: displayName,
                })}
                className="flex h-8 w-8 items-center justify-center p-0 text-text/60 hover:bg-logo-primary/10 hover:text-logo-primary"
              >
                <Settings2 className="h-4 w-4" />
              </Button>
            )}
            {onDelete && (status === "available" || status === "active") && (
              <Button
                variant="ghost"
                size="sm"
                onClick={handleDelete}
                title={t("modelSelector.deleteModel", {
                  modelName: displayName,
                })}
                aria-label={t("modelSelector.deleteModel", {
                  modelName: displayName,
                })}
                className="flex h-8 w-8 items-center justify-center p-0 text-danger/80 hover:bg-danger/10 hover:text-danger"
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </div>

      {/* Download/extract progress */}
      {status === "downloading" && downloadProgress !== undefined && (
        <div className="border-t border-frost-border bg-white/20 px-3 py-2 dark:bg-white/5">
          <div className="h-1 w-full overflow-hidden rounded-full bg-mid-gray/20">
            <div
              className="h-full rounded-full bg-logo-primary transition-all duration-300"
              style={{ width: `${downloadProgress}%` }}
            />
          </div>
          <div className="mt-2 flex items-center justify-between gap-3 text-xs">
            <span className="font-medium text-text/50">
              {t("modelSelector.downloading", {
                percentage: Math.round(downloadProgress),
              })}
            </span>
            <div className="flex items-center gap-2 text-text/50">
              {downloadSpeed !== undefined && downloadSpeed > 0 && (
                <span className="tabular-nums">
                  {t("modelSelector.downloadSpeed", {
                    speed: downloadSpeed.toFixed(1),
                  })}
                </span>
              )}
              {onCancel && (
                <Button
                  variant="danger-ghost"
                  size="sm"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onCancel(model.id);
                  }}
                  aria-label={t("modelSelector.cancelDownload")}
                >
                  {t("modelSelector.cancel")}
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
      {status === "verifying" && (
        <div className="border-t border-frost-border bg-white/20 px-3 py-2 dark:bg-white/5">
          <div className="h-1 w-full overflow-hidden rounded-full bg-mid-gray/20">
            <div className="h-full w-full animate-pulse rounded-full bg-logo-primary" />
          </div>
          <p className="mt-2 text-xs font-medium text-text/50">
            {t("modelSelector.verifyingGeneric")}
          </p>
        </div>
      )}
      {status === "extracting" && (
        <div className="border-t border-frost-border bg-white/20 px-3 py-2 dark:bg-white/5">
          <div className="h-1 w-full overflow-hidden rounded-full bg-mid-gray/20">
            <div className="h-full w-full animate-pulse rounded-full bg-logo-primary" />
          </div>
          <p className="mt-2 text-xs font-medium text-text/50">
            {t("modelSelector.extractingGeneric")}
          </p>
        </div>
      )}
    </div>
  );
};

export default ModelCard;
