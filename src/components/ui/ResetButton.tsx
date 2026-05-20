import React from "react";
import ResetIcon from "../icons/ResetIcon";

interface ResetButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  ariaLabel?: string;
  children?: React.ReactNode;
}

export const ResetButton: React.FC<ResetButtonProps> = React.memo(
  ({ onClick, disabled = false, className = "", ariaLabel, children }) => (
    <button
      type="button"
      aria-label={ariaLabel}
      className={`rounded-md border border-transparent p-1 transition-[background-color,border-color,transform,color] duration-200 ${
        disabled
          ? "opacity-50 cursor-not-allowed text-text/40"
          : "text-text/70 hover:cursor-pointer hover:border-logo-primary/30 hover:bg-logo-primary/10 hover:text-logo-primary active:scale-[0.96] active:bg-logo-primary/15"
      } ${className}`}
      onClick={onClick}
      disabled={disabled}
    >
      {children ?? <ResetIcon />}
    </button>
  ),
);
