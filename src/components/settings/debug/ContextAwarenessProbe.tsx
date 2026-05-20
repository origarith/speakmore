import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ClipboardCheck, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands, type ContextProbeRun } from "@/bindings";
import Badge from "../../ui/Badge";
import { Button, SettingContainer } from "../../ui";

interface ContextAwarenessProbeProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

const CAPTURE_DELAY_SECONDS = 3;
const PREVIEW_MAX_CHARS = 1200;

const textLength = (value: string | null) =>
  value ? Array.from(value).length : 0;

const optionalText = (value: string | null | undefined) =>
  value && value.trim().length > 0 ? value : "-";

const selectionLabel = (run: ContextProbeRun) => {
  if (
    run.selected_location_utf16 === null ||
    run.selected_length_utf16 === null
  ) {
    return "-";
  }

  return `${run.selected_location_utf16}:${run.selected_length_utf16}`;
};

const attributePreview = (value: string | null | undefined) => {
  if (!value) return "";

  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) {
      return parsed.slice(0, 80).join(", ");
    }
  } catch {
    return value;
  }

  return value;
};

const tailPreview = (value: string | null | undefined) => {
  if (!value) return "";

  const chars = Array.from(value);
  if (chars.length <= PREVIEW_MAX_CHARS) {
    return value;
  }

  return `...\n${chars.slice(-PREVIEW_MAX_CHARS).join("")}`;
};

const contextPreviewText = (run: ContextProbeRun | undefined) => {
  if (!run) return "";

  const contextParts = [
    run.before_text ?? "",
    run.selected_text ?? "",
    run.after_text ?? "",
  ];

  if (contextParts.some((part) => part.length > 0)) {
    return contextParts.join("");
  }

  return run.value_text ?? "";
};

export const ContextAwarenessProbe: React.FC<ContextAwarenessProbeProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const [runs, setRuns] = useState<ContextProbeRun[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isClearing, setIsClearing] = useState(false);
  const [captureCountdown, setCaptureCountdown] = useState<number | null>(null);
  const captureTimerRef = useRef<number | null>(null);
  const countdownIntervalRef = useRef<number | null>(null);

  const loadRuns = useCallback(async () => {
    const result = await commands.getContextProbeRuns(10);
    if (result.status === "ok") {
      setRuns(result.data);
      return;
    }

    toast.error(t("settings.debug.contextProbe.errors.load"), {
      description: String(result.error),
    });
  }, [t]);

  useEffect(() => {
    void loadRuns();
  }, [loadRuns]);

  const latest = runs[0];
  const latestTextPreview = useMemo(() => {
    return tailPreview(contextPreviewText(latest));
  }, [latest]);

  useEffect(
    () => () => {
      if (captureTimerRef.current) {
        window.clearTimeout(captureTimerRef.current);
      }
      if (countdownIntervalRef.current) {
        window.clearInterval(countdownIntervalRef.current);
      }
    },
    [],
  );

  const runCapture = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await commands.captureFocusedContextAfterDelay(
        "settings_debug",
        CAPTURE_DELAY_SECONDS * 1000,
      );
      if (result.status === "ok") {
        setRuns((current) => [result.data, ...current].slice(0, 10));
        toast.success(t("settings.debug.contextProbe.captureSuccess"));
      } else {
        toast.error(t("settings.debug.contextProbe.errors.capture"), {
          description: String(result.error),
        });
      }
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  const handleCapture = () => {
    if (captureCountdown !== null) return;

    setCaptureCountdown(CAPTURE_DELAY_SECONDS);
    void runCapture();

    let remaining = CAPTURE_DELAY_SECONDS;
    countdownIntervalRef.current = window.setInterval(() => {
      remaining -= 1;
      setCaptureCountdown(remaining > 0 ? remaining : null);
      if (remaining <= 0 && countdownIntervalRef.current) {
        window.clearInterval(countdownIntervalRef.current);
        countdownIntervalRef.current = null;
      }
    }, 1000);

    captureTimerRef.current = window.setTimeout(() => {
      captureTimerRef.current = null;
      if (countdownIntervalRef.current) {
        window.clearInterval(countdownIntervalRef.current);
        countdownIntervalRef.current = null;
      }
      setCaptureCountdown(null);
    }, CAPTURE_DELAY_SECONDS * 1000);
  };

  const handleClear = async () => {
    setIsClearing(true);
    try {
      const result = await commands.clearContextProbeRuns();
      if (result.status === "ok") {
        setRuns([]);
        toast.success(t("settings.debug.contextProbe.clearSuccess"));
      } else {
        toast.error(t("settings.debug.contextProbe.errors.clear"), {
          description: String(result.error),
        });
      }
    } finally {
      setIsClearing(false);
    }
  };

  return (
    <SettingContainer
      title={t("settings.debug.contextProbe.title")}
      description={t("settings.debug.contextProbe.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="stacked"
    >
      <div className="space-y-3">
        <p className="text-xs leading-relaxed text-warning">
          {t("settings.debug.contextProbe.privacyWarning")}
        </p>

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="primary-soft"
            size="sm"
            onClick={handleCapture}
            disabled={isLoading || isClearing || captureCountdown !== null}
            className="inline-flex items-center gap-2"
          >
            {isLoading || captureCountdown !== null ? (
              <RefreshCw className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <ClipboardCheck className="h-3.5 w-3.5" />
            )}
            {captureCountdown !== null
              ? t("settings.debug.contextProbe.captureCountdown", {
                  seconds: captureCountdown,
                })
              : t("settings.debug.contextProbe.capture")}
          </Button>
          <Button
            type="button"
            variant="danger-ghost"
            size="sm"
            onClick={handleClear}
            disabled={isLoading || isClearing || runs.length === 0}
            className="inline-flex items-center gap-2"
          >
            <Trash2 className="h-3.5 w-3.5" />
            {t("settings.debug.contextProbe.clear")}
          </Button>
        </div>

        {captureCountdown !== null && (
          <p
            className="text-xs leading-relaxed text-text/60"
            aria-live="polite"
          >
            {t("settings.debug.contextProbe.captureArmed", {
              seconds: captureCountdown,
            })}
          </p>
        )}

        {latest ? (
          <div className="space-y-3">
            <div className="grid gap-2 text-xs sm:grid-cols-2">
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.app")}
                value={optionalText(latest.app_name)}
              />
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.window")}
                value={optionalText(latest.window_title)}
              />
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.role")}
                value={optionalText(latest.element_role)}
              />
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.selection")}
                value={selectionLabel(latest)}
              />
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.textLength")}
                value={String(textLength(latest.value_text))}
              />
              <ProbeFact
                label={t("settings.debug.contextProbe.fields.latency")}
                value={t("settings.debug.contextProbe.latencyValue", {
                  value: latest.latency_ms,
                })}
              />
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Badge
                variant={latest.status === "success" ? "success" : "warning"}
              >
                {t(`settings.debug.contextProbe.status.${latest.status}`)}
              </Badge>
              <Badge variant="secondary">
                {t(
                  `settings.debug.contextProbe.confidence.${latest.confidence}`,
                )}
              </Badge>
              {latest.truncated && (
                <Badge variant="warning">
                  {t("settings.debug.contextProbe.truncated")}
                </Badge>
              )}
            </div>

            {latest.failure_reason && (
              <p
                className={`text-xs leading-relaxed ${
                  latest.status === "success" ? "text-warning" : "text-danger"
                }`}
              >
                {latest.failure_reason}
              </p>
            )}

            {latest.available_attributes_json && (
              <div className="space-y-1">
                <h4 className="text-xs font-semibold text-text/60">
                  {t("settings.debug.contextProbe.attributes")}
                </h4>
                <pre className="max-h-24 overflow-auto rounded-md border border-frost-border bg-white/40 p-2 text-[11px] leading-relaxed text-text/65 dark:bg-white/5">
                  {attributePreview(latest.available_attributes_json)}
                </pre>
              </div>
            )}

            <pre className="max-h-40 overflow-auto rounded-md border border-frost-border bg-white/40 p-2 text-xs leading-relaxed text-text/80 dark:bg-white/5">
              {latestTextPreview || t("settings.debug.contextProbe.noText")}
            </pre>
          </div>
        ) : (
          <p className="text-xs text-text/55">
            {t("settings.debug.contextProbe.empty")}
          </p>
        )}

        {runs.length > 0 && (
          <div className="space-y-2">
            <h4 className="text-xs font-semibold text-text/60">
              {t("settings.debug.contextProbe.recent")}
            </h4>
            <div className="max-h-56 overflow-auto rounded-md border border-frost-border">
              {runs.map((run) => (
                <div
                  key={run.id}
                  className="grid gap-1 border-b border-frost-border px-3 py-2 text-xs last:border-b-0 sm:grid-cols-[1fr_auto]"
                >
                  <div className="min-w-0">
                    <div className="truncate font-medium">
                      {optionalText(run.app_name)}{" "}
                      {t("settings.debug.contextProbe.separator")}{" "}
                      {optionalText(run.window_title)}
                    </div>
                    <div className="truncate text-text/55">
                      {optionalText(run.element_role)}{" "}
                      {t("settings.debug.contextProbe.separator")}{" "}
                      {selectionLabel(run)}
                    </div>
                  </div>
                  <div className="flex flex-wrap items-center gap-2 sm:justify-end">
                    <span className="text-text/55">
                      {t("settings.debug.contextProbe.textLengthValue", {
                        value: textLength(run.value_text),
                      })}
                    </span>
                    <Badge
                      variant={
                        run.status === "success" ? "success" : "secondary"
                      }
                    >
                      {t(`settings.debug.contextProbe.status.${run.status}`)}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};

const ProbeFact: React.FC<{ label: string; value: string }> = ({
  label,
  value,
}) => (
  <div className="min-w-0 rounded-md border border-frost-border px-2 py-1.5">
    <div className="text-text/45">{label}</div>
    <div className="truncate font-medium text-text/80">{value}</div>
  </div>
);
