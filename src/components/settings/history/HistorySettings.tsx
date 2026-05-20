import React, { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readFile } from "@tauri-apps/plugin-fs";
import {
  Check,
  Copy,
  FilePenLine,
  FolderOpen,
  Info,
  RotateCcw,
  Save,
  Star,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  commands,
  events,
  type HistoryEntry,
  type HistoryEntryDetail,
  type HistoryUpdatePayload,
} from "@/bindings";
import { useOsType } from "@/hooks/useOsType";
import { formatDateTime } from "@/utils/dateFormat";
import { AudioPlayer } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";
import { Textarea } from "../../ui/Textarea";

const IconButton: React.FC<{
  onClick: () => void;
  title: string;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}> = ({ onClick, title, disabled, active, children }) => (
  <button
    onClick={onClick}
    disabled={disabled}
    className={`p-1.5 rounded-md flex items-center justify-center transition-colors cursor-pointer disabled:cursor-not-allowed disabled:text-text/20 ${
      active
        ? "text-logo-primary hover:text-logo-primary/80"
        : "text-text/50 hover:text-logo-primary"
    }`}
    title={title}
  >
    {children}
  </button>
);

const PAGE_SIZE = 30;
type HistoryTextLayer = "user" | "final" | "raw";

interface OpenRecordingsButtonProps {
  onClick: () => void;
  label: string;
}

const OpenRecordingsButton: React.FC<OpenRecordingsButtonProps> = ({
  onClick,
  label,
}) => (
  <Button
    onClick={onClick}
    variant="secondary"
    size="sm"
    className="flex items-center gap-2"
    title={label}
  >
    <FolderOpen className="w-4 h-4" />
    <span>{label}</span>
  </Button>
);

export const HistorySettings: React.FC = () => {
  const { t } = useTranslation();
  const osType = useOsType();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);

  // Keep ref in sync for use in IntersectionObserver callback
  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  const loadPage = useCallback(async (cursor?: number) => {
    const isFirstPage = cursor === undefined;
    if (!isFirstPage && loadingRef.current) return;
    loadingRef.current = true;

    if (isFirstPage) setLoading(true);

    try {
      const result = await commands.getHistoryEntries(
        cursor ?? null,
        PAGE_SIZE,
      );
      if (result.status === "ok") {
        const { entries: newEntries, has_more } = result.data;
        setEntries((prev) =>
          isFirstPage ? newEntries : [...prev, ...newEntries],
        );
        setHasMore(has_more);
      }
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadPage();
  }, [loadPage]);

  // Infinite scroll via IntersectionObserver
  useEffect(() => {
    if (loading) return;

    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;

    const observer = new IntersectionObserver(
      (observerEntries) => {
        const first = observerEntries[0];
        if (first.isIntersecting) {
          const lastEntry = entriesRef.current[entriesRef.current.length - 1];
          if (lastEntry) {
            loadPage(lastEntry.id);
          }
        }
      },
      { threshold: 0 },
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loading, hasMore, loadPage]);

  // Listen for new entries added from the transcription pipeline
  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen((event) => {
      const payload: HistoryUpdatePayload = event.payload;
      if (payload.action === "added") {
        setEntries((prev) => [payload.entry, ...prev]);
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
      }
      // "deleted" and "toggled" are handled by optimistic updates only,
      // so we intentionally ignore them here to avoid double-mutation.
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const toggleSaved = async (id: number) => {
    // Optimistic update
    setEntries((prev) =>
      prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
    );
    try {
      const result = await commands.toggleHistoryEntrySaved(id);
      if (result.status !== "ok") {
        // Revert on failure
        setEntries((prev) =>
          prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
        );
      }
    } catch (error) {
      console.error("Failed to toggle saved status:", error);
      // Revert on failure
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
      );
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
      return false;
    }
  };

  const getAudioUrl = useCallback(
    async (fileName: string) => {
      try {
        const result = await commands.getAudioFilePath(fileName);
        if (result.status === "ok") {
          if (osType === "linux") {
            const fileData = await readFile(result.data);
            const blob = new Blob([fileData], { type: "audio/wav" });
            return URL.createObjectURL(blob);
          }
          return convertFileSrc(result.data, "asset");
        }
        return null;
      } catch (error) {
        console.error("Failed to get audio file path:", error);
        return null;
      }
    },
    [osType],
  );

  const deleteAudioEntry = async (id: number) => {
    // Optimistically remove
    setEntries((prev) => prev.filter((e) => e.id !== id));
    try {
      const result = await commands.deleteHistoryEntry(id);
      if (result.status !== "ok") {
        // Reload on failure
        loadPage();
      }
    } catch (error) {
      console.error("Failed to delete entry:", error);
      loadPage();
    }
  };

  const retryHistoryEntry = async (id: number) => {
    const result = await commands.retryHistoryEntryTranscription(id);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }
  };

  const updateUserEdit = async (id: number, text: string | null) => {
    const result = await commands.updateHistoryEntryUserEdit(id, text);
    if (result.status !== "ok") {
      throw new Error(String(result.error));
    }

    setEntries((prev) =>
      prev.map((entry) => (entry.id === id ? result.data : entry)),
    );
  };

  const openRecordingsFolder = async () => {
    try {
      const result = await commands.openRecordingsFolder();
      if (result.status !== "ok") {
        throw new Error(String(result.error));
      }
    } catch (error) {
      console.error("Failed to open recordings folder:", error);
    }
  };

  let content: React.ReactNode;

  if (loading) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.history.loading")}
      </div>
    );
  } else if (entries.length === 0) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.history.empty")}
      </div>
    );
  } else {
    content = (
      <>
        <div className="divide-y divide-mid-gray/20">
          {entries.map((entry) => (
            <HistoryEntryComponent
              key={entry.id}
              entry={entry}
              onToggleSaved={() => toggleSaved(entry.id)}
              copyText={copyToClipboard}
              getAudioUrl={getAudioUrl}
              deleteAudio={deleteAudioEntry}
              retryTranscription={retryHistoryEntry}
              updateUserEdit={updateUserEdit}
            />
          ))}
        </div>
        {/* Sentinel for infinite scroll */}
        <div ref={sentinelRef} className="h-1" />
      </>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="space-y-2">
        <div className="px-4 flex items-center justify-between">
          <div>
            <h2 className="text-[11px] font-semibold uppercase text-text/50">
              {t("settings.history.title")}
            </h2>
          </div>
          <OpenRecordingsButton
            onClick={openRecordingsFolder}
            label={t("settings.history.openFolder")}
          />
        </div>
        <div className="frost-surface overflow-visible rounded-lg">
          {content}
        </div>
      </div>
    </div>
  );
};

interface HistoryEntryProps {
  entry: HistoryEntry;
  onToggleSaved: () => void;
  copyText: (text: string) => Promise<boolean>;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
  retryTranscription: (id: number) => Promise<void>;
  updateUserEdit: (id: number, text: string | null) => Promise<void>;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  onToggleSaved,
  copyText,
  getAudioUrl,
  deleteAudio,
  retryTranscription,
  updateUserEdit,
}) => {
  const { t, i18n } = useTranslation();
  const [showCopied, setShowCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [textLayer, setTextLayer] = useState<HistoryTextLayer>(() =>
    entry.user_edited_text?.trim() ? "user" : "final",
  );
  const previousEntryIdRef = useRef(entry.id);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detail, setDetail] = useState<HistoryEntryDetail | null>(null);
  const [editing, setEditing] = useState(false);
  const [editDraft, setEditDraft] = useState("");
  const [savingEdit, setSavingEdit] = useState(false);

  const userEditedText = entry.user_edited_text ?? "";
  const generatedFinalText =
    entry.post_processed_text || entry.transcription_text;
  const hasUserEdited = userEditedText.trim().length > 0;
  const hasPostProcessed = Boolean(entry.post_processed_text?.trim());
  const finalText =
    entry.final_text || userEditedText || generatedFinalText || "";
  const hasFinalText = finalText.trim().length > 0;
  const displayedText =
    textLayer === "user" && hasUserEdited
      ? userEditedText
      : textLayer === "raw"
        ? entry.transcription_text
        : generatedFinalText;
  const displayedTextIsRaw = textLayer === "raw" || !hasPostProcessed;
  const displayedTextIsUser = textLayer === "user" && hasUserEdited;
  const hasLayerSwitch = hasUserEdited || hasPostProcessed;
  const textLayerOptions: Array<{
    value: HistoryTextLayer;
    label: string;
    visible: boolean;
  }> = [
    {
      value: "user",
      label: t("settings.history.userText"),
      visible: hasUserEdited,
    },
    {
      value: "final",
      label: t("settings.history.finalText"),
      visible: true,
    },
    {
      value: "raw",
      label: t("settings.history.rawText"),
      visible: hasPostProcessed || hasUserEdited,
    },
  ];

  useEffect(() => {
    if (previousEntryIdRef.current !== entry.id) {
      previousEntryIdRef.current = entry.id;
      setTextLayer(hasUserEdited ? "user" : "final");
      setEditing(false);
      setEditDraft("");
      return;
    }

    if (!hasUserEdited && textLayer === "user") {
      setTextLayer("final");
    }
  }, [entry.id, hasUserEdited, textLayer]);

  const handleLoadAudio = useCallback(
    () => getAudioUrl(entry.file_name),
    [getAudioUrl, entry.file_name],
  );

  const handleCopyText = async () => {
    if (!displayedText.trim()) {
      return;
    }

    const copied = await copyText(displayedText);
    if (!copied) {
      return;
    }

    const eventType = displayedTextIsUser
      ? "copied_user_text"
      : displayedTextIsRaw
        ? "copied_raw_text"
        : "copied_final_text";
    const runType = displayedTextIsUser
      ? null
      : displayedTextIsRaw
        ? "transcription"
        : "post_process";
    const runId = displayedTextIsUser
      ? null
      : displayedTextIsRaw
        ? entry.latest_transcription_run_id
        : entry.latest_post_process_run_id;
    const payloadJson = displayedTextIsUser
      ? JSON.stringify({ text_len: displayedText.length })
      : null;
    void commands.recordHistoryEvent(
      entry.id,
      eventType,
      runType,
      runId,
      "frontend",
      payloadJson,
    );
    setShowCopied(true);
    setTimeout(() => setShowCopied(false), 2000);
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      toast.error(t("settings.history.deleteError"));
    }
  };

  const handleRetranscribe = async () => {
    try {
      setRetrying(true);
      await retryTranscription(entry.id);
      setDetail(null);
    } catch (error) {
      console.error("Failed to re-transcribe:", error);
      toast.error(t("settings.history.retranscribeError"));
    } finally {
      setRetrying(false);
    }
  };

  const handleStartEdit = () => {
    setEditDraft(hasUserEdited ? userEditedText : generatedFinalText);
    setEditing(true);
  };

  const handleCancelEdit = () => {
    setEditing(false);
    setEditDraft("");
  };

  const handleSaveEdit = async () => {
    if (!editDraft.trim()) return;

    try {
      setSavingEdit(true);
      await updateUserEdit(entry.id, editDraft);
      setTextLayer("user");
      setDetail(null);
      setEditing(false);
    } catch (error) {
      console.error("Failed to save history edit:", error);
      toast.error(t("settings.history.editError"));
    } finally {
      setSavingEdit(false);
    }
  };

  const handleClearEdit = async () => {
    try {
      setSavingEdit(true);
      await updateUserEdit(entry.id, null);
      setTextLayer("final");
      setDetail(null);
      setEditing(false);
      setEditDraft("");
    } catch (error) {
      console.error("Failed to clear history edit:", error);
      toast.error(t("settings.history.clearEditError"));
    } finally {
      setSavingEdit(false);
    }
  };

  const toggleDetail = async () => {
    const nextOpen = !detailOpen;
    setDetailOpen(nextOpen);
    if (!nextOpen || detail || detailLoading) {
      return;
    }

    try {
      setDetailLoading(true);
      const result = await commands.getHistoryEntryDetail(entry.id);
      if (result.status === "ok") {
        setDetail(result.data);
      } else {
        throw new Error(String(result.error));
      }
    } catch (error) {
      console.error("Failed to load history detail:", error);
      toast.error(t("settings.history.detailError"));
    } finally {
      setDetailLoading(false);
    }
  };

  const formattedDate = formatDateTime(String(entry.timestamp), i18n.language);
  const formatValue = (value?: string | number | null) =>
    value === undefined || value === null || value === "" ? "-" : String(value);

  return (
    <div className="px-4 py-2 pb-5 flex flex-col gap-3">
      <div className="flex justify-between items-center">
        <p className="text-sm font-medium">{formattedDate}</p>
        <div className="flex items-center">
          <IconButton
            onClick={handleCopyText}
            disabled={!hasFinalText || retrying}
            title={t("settings.history.copyToClipboard")}
          >
            {showCopied ? (
              <Check width={16} height={16} />
            ) : (
              <Copy width={16} height={16} />
            )}
          </IconButton>
          <IconButton
            onClick={handleStartEdit}
            disabled={!hasFinalText || retrying || savingEdit}
            active={editing}
            title={t("settings.history.edit")}
          >
            <FilePenLine width={16} height={16} />
          </IconButton>
          <IconButton
            onClick={onToggleSaved}
            disabled={retrying}
            active={entry.saved}
            title={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
          >
            <Star
              width={16}
              height={16}
              fill={entry.saved ? "currentColor" : "none"}
            />
          </IconButton>
          <IconButton
            onClick={handleRetranscribe}
            disabled={retrying}
            title={t("settings.history.retranscribe")}
          >
            <RotateCcw
              width={16}
              height={16}
              style={
                retrying
                  ? { animation: "spin 1s linear infinite reverse" }
                  : undefined
              }
            />
          </IconButton>
          <IconButton
            onClick={toggleDetail}
            disabled={retrying}
            active={detailOpen}
            title={t("settings.history.details")}
          >
            <Info width={16} height={16} />
          </IconButton>
          <IconButton
            onClick={handleDeleteEntry}
            disabled={retrying}
            title={t("settings.history.delete")}
          >
            <Trash2 width={16} height={16} />
          </IconButton>
        </div>
      </div>

      <div className="flex flex-wrap gap-1.5 text-[11px] text-text/55">
        <span className="rounded-md bg-background/60 px-2 py-1">
          {t("settings.history.status")}: {formatValue(entry.status)}
        </span>
        <span className="rounded-md bg-background/60 px-2 py-1">
          {t("settings.history.asr")}: {formatValue(entry.asr_provider_id)}
        </span>
        <span className="rounded-md bg-background/60 px-2 py-1">
          {t("settings.history.model")}: {formatValue(entry.asr_model)}
        </span>
        <span className="rounded-md bg-background/60 px-2 py-1">
          {t("settings.history.language")}: {formatValue(entry.asr_language)}
        </span>
        {entry.post_process_preset_id && (
          <span className="rounded-md bg-background/60 px-2 py-1">
            {t("settings.history.preset")}:{" "}
            {formatValue(entry.post_process_preset_id)}
          </span>
        )}
      </div>

      {hasLayerSwitch && (
        <div className="inline-flex w-fit rounded-md border border-mid-gray/20 bg-background/50 p-0.5 text-xs">
          {textLayerOptions
            .filter((option) => option.visible)
            .map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => setTextLayer(option.value)}
                className={`rounded px-2 py-1 transition-colors ${
                  textLayer === option.value
                    ? "bg-logo-primary text-white"
                    : "text-text/60"
                }`}
              >
                {option.label}
              </button>
            ))}
        </div>
      )}

      {editing && (
        <div className="space-y-2 rounded-md border border-mid-gray/20 bg-background/50 p-2">
          <Textarea
            value={editDraft}
            onChange={(event) => setEditDraft(event.target.value)}
            variant="compact"
            className="min-h-[120px] w-full"
            disabled={savingEdit}
            aria-label={t("settings.history.editText")}
          />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="primary"
              size="sm"
              onClick={handleSaveEdit}
              disabled={!editDraft.trim() || savingEdit}
              className="inline-flex items-center gap-1.5"
            >
              <Save className="h-3.5 w-3.5" />
              <span>{t("settings.history.saveEdit")}</span>
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={handleCancelEdit}
              disabled={savingEdit}
              className="inline-flex items-center gap-1.5"
            >
              <X className="h-3.5 w-3.5" />
              <span>{t("settings.history.cancelEdit")}</span>
            </Button>
            {hasUserEdited && (
              <Button
                variant="ghost"
                size="sm"
                onClick={handleClearEdit}
                disabled={savingEdit}
                className="inline-flex items-center gap-1.5"
              >
                <Undo2 className="h-3.5 w-3.5" />
                <span>{t("settings.history.restoreGenerated")}</span>
              </Button>
            )}
          </div>
        </div>
      )}

      <p
        className={`italic text-sm pb-2 ${
          retrying
            ? ""
            : displayedText.trim()
              ? "text-text/90 select-text cursor-text whitespace-pre-wrap break-words"
              : "text-text/40"
        }`}
        style={
          retrying
            ? { animation: "transcribe-pulse 3s ease-in-out infinite" }
            : undefined
        }
      >
        {retrying && (
          <style>{`
            @keyframes transcribe-pulse {
              0%, 100% { color: color-mix(in srgb, var(--color-text) 40%, transparent); }
              50% { color: color-mix(in srgb, var(--color-text) 90%, transparent); }
            }
          `}</style>
        )}
        {retrying
          ? t("settings.history.transcribing")
          : displayedText.trim()
            ? displayedText
            : t("settings.history.transcriptionFailed")}
      </p>

      {detailOpen && (
        <HistoryDetailPanel
          detail={detail}
          loading={detailLoading}
          formatValue={formatValue}
        />
      )}

      <AudioPlayer onLoadRequest={handleLoadAudio} className="w-full" />
    </div>
  );
};

interface HistoryDetailPanelProps {
  detail: HistoryEntryDetail | null;
  loading: boolean;
  formatValue: (value?: string | number | null) => string;
}

const HistoryDetailPanel: React.FC<HistoryDetailPanelProps> = ({
  detail,
  loading,
  formatValue,
}) => {
  const { t, i18n } = useTranslation();

  if (loading) {
    return (
      <div className="rounded-md bg-background/50 px-3 py-2 text-xs text-text/60">
        {t("settings.history.detailLoading")}
      </div>
    );
  }

  if (!detail) {
    return null;
  }

  return (
    <div className="rounded-md bg-background/50 px-3 py-2 text-xs text-text/70">
      <p className="mb-2 font-medium text-text/80">
        {t("settings.history.details")}
      </p>
      <HistoryDetailSection title={t("settings.history.transcriptionRuns")}>
        {detail.transcription_runs.length === 0 ? (
          <HistoryDetailEmpty />
        ) : (
          detail.transcription_runs.map((run) => (
            <HistoryDetailRow
              key={run.id}
              title={`${formatValue(run.provider_id)} / ${formatValue(run.model)}`}
              timestamp={run.created_at}
              status={run.status}
              meta={[
                `${t("settings.history.language")}: ${formatValue(run.language)}`,
                `${t("settings.history.latency")}: ${run.latency_ms}${t("settings.history.ms")}`,
                run.error_summary
                  ? `${t("settings.history.error")}: ${run.error_summary}`
                  : null,
              ]}
              locale={i18n.language}
            />
          ))
        )}
      </HistoryDetailSection>
      <HistoryDetailSection title={t("settings.history.postProcessRuns")}>
        {detail.post_process_runs.length === 0 ? (
          <HistoryDetailEmpty />
        ) : (
          detail.post_process_runs.map((run) => (
            <HistoryDetailRow
              key={run.id}
              title={`${formatValue(run.preset_id)} / ${formatValue(run.model)}`}
              timestamp={run.created_at}
              status={run.status}
              meta={[
                `${t("settings.history.provider")}: ${formatValue(run.provider_id)}`,
                `${t("settings.history.latency")}: ${run.latency_ms}${t("settings.history.ms")}`,
                run.error_summary
                  ? `${t("settings.history.error")}: ${run.error_summary}`
                  : null,
              ]}
              locale={i18n.language}
            />
          ))
        )}
      </HistoryDetailSection>
      <HistoryDetailSection title={t("settings.history.events")}>
        {detail.events.length === 0 ? (
          <HistoryDetailEmpty />
        ) : (
          detail.events.map((event) => (
            <HistoryDetailRow
              key={event.id}
              title={event.event_type.replace(/_/g, " ")}
              timestamp={event.created_at}
              status={event.source}
              meta={[
                `${t("settings.history.run")}: ${formatValue(event.run_type)}`,
                `${t("settings.history.runId")}: ${formatValue(event.run_id)}`,
              ]}
              locale={i18n.language}
            />
          ))
        )}
      </HistoryDetailSection>
    </div>
  );
};

const HistoryDetailSection: React.FC<{
  title: string;
  children: React.ReactNode;
}> = ({ title, children }) => (
  <div className="border-t border-mid-gray/20 py-2 first:border-t-0 first:pt-0">
    <p className="mb-1 text-[11px] font-semibold uppercase text-text/45">
      {title}
    </p>
    <div className="space-y-1.5">{children}</div>
  </div>
);

const HistoryDetailEmpty: React.FC = () => {
  const { t } = useTranslation();
  return <p className="text-text/40">{t("settings.history.noDetailItems")}</p>;
};

const HistoryDetailRow: React.FC<{
  title: string;
  timestamp: number;
  status: string;
  meta: Array<string | null>;
  locale: string;
}> = ({ title, timestamp, status, meta, locale }) => (
  <div className="rounded-md border border-mid-gray/10 bg-white/30 px-2 py-1.5">
    <div className="flex flex-wrap items-center justify-between gap-2">
      <span className="font-medium text-text/75">{title}</span>
      <span className="rounded bg-background/70 px-1.5 py-0.5 text-[10px] uppercase text-text/50">
        {status}
      </span>
    </div>
    <p className="mt-0.5 text-[11px] text-text/45">
      {formatDateTime(String(timestamp), locale)}
    </p>
    <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-text/55">
      {meta
        .filter((item): item is string => Boolean(item))
        .map((item) => (
          <span key={item}>{item}</span>
        ))}
    </div>
  </div>
);
