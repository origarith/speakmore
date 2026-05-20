import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, Mic, Settings, Sparkles, X } from "lucide-react";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";

type OverlayState = "recording" | "transcribing" | "processing" | "realtime";

type RealtimeTranscriptionUpdate = {
  status: "connecting" | "listening" | "transcribing" | "finalizing" | "error";
  final_text: string;
  partial_text: string;
};

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const [realtimeUpdate, setRealtimeUpdate] =
    useState<RealtimeTranscriptionUpdate>({
      status: "connecting",
      final_text: "",
      partial_text: "",
    });
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const realtimeTextRef = useRef<HTMLDivElement | null>(null);
  const direction = getLanguageDirection(i18n.language);

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        if (overlayState === "realtime") {
          setRealtimeUpdate({
            status: "connecting",
            final_text: "",
            partial_text: "",
          });
        }
        setIsVisible(true);
      });

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
      });

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      const unlistenRealtime = await listen<RealtimeTranscriptionUpdate>(
        "realtime-transcription-update",
        (event) => {
          setRealtimeUpdate(event.payload);
        },
      );

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
        unlistenRealtime();
      };
    };

    setupEventListeners();
  }, []);

  useEffect(() => {
    const el = realtimeTextRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [realtimeUpdate.final_text, realtimeUpdate.partial_text]);

  const getIcon = () => {
    if (state === "recording") return <Mic size={16} strokeWidth={2.4} />;
    if (state === "processing") return <Sparkles size={16} strokeWidth={2.4} />;
    return <FileText size={16} strokeWidth={2.4} />;
  };

  const getStateLabel = () => {
    if (state === "recording") return t("overlay.recording");
    if (state === "processing") return t("overlay.processing");
    if (state === "transcribing") return t("overlay.transcribing");
    return t(`overlay.realtime.${realtimeUpdate.status}`);
  };

  return (
    <div className="recording-overlay-shell" dir={direction}>
      <div
        className={`recording-overlay ${
          state === "realtime" ? "recording-overlay-realtime" : ""
        } ${isVisible ? "fade-in" : ""}`}
      >
        <div className="overlay-left">
          <span className={`overlay-icon overlay-icon-${state}`}>
            {getIcon()}
          </span>
        </div>

        <div className="overlay-middle">
          {state === "recording" && (
            <div className="compact-state">
              <span className="state-dot" />
              <div className="bars-container">
                {levels.map((v, i) => (
                  <div
                    key={i}
                    className="bar"
                    style={{
                      transform: `scaleY(${Math.min(1, 0.2 + Math.pow(v, 0.7) * 0.8)})`,
                      opacity: Math.max(0.24, v * 1.7),
                    }}
                  />
                ))}
              </div>
            </div>
          )}
          {state === "transcribing" && (
            <div className="transcribing-text">{getStateLabel()}</div>
          )}
          {state === "processing" && (
            <div className="transcribing-text">{getStateLabel()}</div>
          )}
          {state === "realtime" && (
            <div className="realtime-panel">
              <div className="realtime-header">
                <span className="state-dot" />
                <span className="realtime-status">{getStateLabel()}</span>
                <Settings className="realtime-header-icon" size={14} />
              </div>
              <div ref={realtimeTextRef} className="realtime-text">
                <span>{realtimeUpdate.final_text}</span>
                {realtimeUpdate.partial_text && (
                  <span className="realtime-partial">
                    {realtimeUpdate.partial_text}
                  </span>
                )}
                {!realtimeUpdate.final_text && !realtimeUpdate.partial_text && (
                  <span className="realtime-placeholder">
                    {t("overlay.realtime.waiting")}
                  </span>
                )}
              </div>
            </div>
          )}
        </div>

        <div className="overlay-right">
          {(state === "recording" || state === "realtime") && (
            <button
              type="button"
              className="cancel-button"
              onClick={() => {
                commands.cancelOperation();
              }}
              aria-label={t("common.cancel")}
            >
              <X size={14} strokeWidth={2.4} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default RecordingOverlay;
