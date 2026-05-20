import React from "react";
import { ChevronDown } from "lucide-react";

type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "verifying"
  | "extracting"
  | "error"
  | "unloaded"
  | "none";

interface ModelStatusButtonProps {
  status: ModelStatus;
  displayText: string;
  isDropdownOpen: boolean;
  onClick: () => void;
  className?: string;
  disabled?: boolean;
  showDropdownIndicator?: boolean;
  title?: string;
}

const ModelStatusButton: React.FC<ModelStatusButtonProps> = ({
  status,
  displayText,
  isDropdownOpen,
  onClick,
  className = "",
  disabled = false,
  showDropdownIndicator = true,
  title,
}) => {
  const getStatusColor = (status: ModelStatus): string => {
    switch (status) {
      case "ready":
        return "bg-success";
      case "loading":
        return "bg-warning animate-pulse";
      case "downloading":
        return "bg-logo-primary animate-pulse";
      case "verifying":
        return "bg-accent animate-pulse";
      case "extracting":
        return "bg-accent animate-pulse";
      case "error":
        return "bg-danger";
      case "unloaded":
        return "bg-mid-gray/60";
      case "none":
        return "bg-danger";
      default:
        return "bg-mid-gray/60";
    }
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`inline-flex min-h-8 items-center gap-2 rounded-md border border-frost-border bg-white/45 px-2.5 text-xs font-medium text-text/70 shadow-sm transition-[background-color,border-color,color,box-shadow,transform] duration-200 dark:bg-white/5 ${
        disabled
          ? "cursor-default"
          : "hover:border-logo-primary/30 hover:bg-logo-primary/10 hover:text-logo-primary active:scale-[0.98]"
      } ${className}`}
      title={title ?? displayText}
    >
      <div className={`h-2 w-2 rounded-full ${getStatusColor(status)}`} />
      <span className="max-w-28 truncate">{displayText}</span>
      {showDropdownIndicator && (
        <ChevronDown
          className={`h-3 w-3 transition-transform duration-200 ${isDropdownOpen ? "rotate-180" : ""}`}
        />
      )}
    </button>
  );
};

export default ModelStatusButton;
