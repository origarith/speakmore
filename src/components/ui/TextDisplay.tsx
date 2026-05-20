import React, { useState } from "react";
import { SettingContainer } from "./SettingContainer";

interface TextDisplayProps {
  label: string;
  description: string;
  value: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  placeholder?: string;
  copyable?: boolean;
  monospace?: boolean;
  onCopy?: (value: string) => void;
}

export const TextDisplay: React.FC<TextDisplayProps> = ({
  label,
  description,
  value,
  descriptionMode = "tooltip",
  grouped = false,
  placeholder = "Not available",
  copyable = false,
  monospace = false,
  onCopy,
}) => {
  const [showCopied, setShowCopied] = useState(false);

  const handleCopy = async () => {
    if (!value || !copyable) return;

    try {
      await navigator.clipboard.writeText(value);
      setShowCopied(true);
      setTimeout(() => setShowCopied(false), 1500);
      if (onCopy) {
        onCopy(value);
      }
    } catch (err) {
      console.error("Failed to copy to clipboard:", err);
    }
  };

  const displayValue = value || placeholder;
  const textClasses = monospace ? "font-mono break-all" : "break-words";

  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="stacked"
    >
      <div className="flex items-center space-x-2">
        <div className="flex-1 min-w-0">
          <div
            className={`frost-control flex min-h-8 items-center rounded-md border px-2 text-xs ${textClasses} ${!value ? "opacity-60" : ""}`}
          >
            {displayValue}
          </div>
        </div>
        {copyable && value && (
          <button
            onClick={handleCopy}
            className="frost-control flex min-h-8 w-12 shrink-0 cursor-pointer items-center justify-center rounded-md border px-2 py-1 text-xs font-semibold transition-all duration-150 hover:border-logo-primary/35 hover:text-logo-primary"
            title="Copy to clipboard"
          >
            {showCopied ? (
              <div className="flex items-center space-x-1">
                <svg
                  className="w-4 h-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M5 13l4 4L19 7"
                  />
                </svg>
              </div>
            ) : (
              "Copy"
            )}
          </button>
        )}
      </div>
    </SettingContainer>
  );
};
