import React from "react";
import { SettingContainer } from "./SettingContainer";

interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  isUpdating?: boolean;
  label: string;
  description: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  tooltipPosition?: "top" | "bottom";
}

export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({
  checked,
  onChange,
  disabled = false,
  isUpdating = false,
  label,
  description,
  descriptionMode = "tooltip",
  grouped = false,
  tooltipPosition = "top",
}) => {
  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={disabled}
      tooltipPosition={tooltipPosition}
    >
      <label
        className={`inline-flex items-center ${disabled || isUpdating ? "cursor-not-allowed" : "cursor-pointer"}`}
      >
        <input
          type="checkbox"
          value=""
          className="sr-only peer"
          checked={checked}
          disabled={disabled || isUpdating}
          onChange={(e) => onChange(e.target.checked)}
        />
        <div
          className={`relative h-6 w-11 shrink-0 rounded-full border transition-[background-color,border-color,box-shadow] duration-200 ${
            checked
              ? "border-background-ui bg-background-ui shadow-[0_6px_16px_rgba(47,143,131,0.22)]"
              : "border-black/10 bg-black/10 shadow-inner dark:border-white/10 dark:bg-white/10"
          } peer-focus:ring-2 peer-focus:ring-logo-primary/25 peer-disabled:opacity-50`}
        >
          <span
            aria-hidden="true"
            className="absolute top-1/2 h-5 w-5 -translate-y-1/2 rounded-full border border-black/10 bg-white shadow-sm transition-[inset-inline-start,box-shadow,transform] duration-200"
            style={{ insetInlineStart: checked ? "22px" : "2px" }}
          />
        </div>
      </label>
      {isUpdating && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-logo-primary border-t-transparent rounded-full animate-spin"></div>
        </div>
      )}
    </SettingContainer>
  );
};
