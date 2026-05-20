import React, { useEffect, useRef, useState } from "react";
import { Info } from "lucide-react";
import { Tooltip } from "./Tooltip";

interface SettingContainerProps {
  title: string;
  description: string;
  children: React.ReactNode;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  layout?: "horizontal" | "stacked";
  disabled?: boolean;
  tooltipPosition?: "top" | "bottom";
}

export const SettingContainer: React.FC<SettingContainerProps> = ({
  title,
  description,
  children,
  descriptionMode = "tooltip",
  grouped = false,
  layout = "horizontal",
  disabled = false,
  tooltipPosition = "top",
}) => {
  const [showTooltip, setShowTooltip] = useState(false);
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Handle click outside to close tooltip
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        tooltipRef.current &&
        !tooltipRef.current.contains(event.target as Node)
      ) {
        setShowTooltip(false);
      }
    };

    if (showTooltip) {
      document.addEventListener("mousedown", handleClickOutside);
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [showTooltip]);

  const toggleTooltip = () => {
    setShowTooltip(!showTooltip);
  };

  const containerClasses = grouped
    ? "px-4 py-3"
    : "frost-surface rounded-lg px-4 py-3 transition-[background-color,border-color,box-shadow,transform] duration-200 hover:border-logo-primary/20";

  if (layout === "stacked") {
    if (descriptionMode === "tooltip") {
      return (
        <div className={containerClasses}>
          <div className="mb-2 flex items-center gap-2">
            <h3
              className={`text-sm font-semibold ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            <div
              ref={tooltipRef}
              className="relative"
              onMouseEnter={() => setShowTooltip(true)}
              onMouseLeave={() => setShowTooltip(false)}
              onClick={toggleTooltip}
            >
              <Info
                className="h-4 w-4 cursor-help select-none text-mid-gray transition-colors duration-150 hover:text-logo-primary"
                aria-label="More information"
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleTooltip();
                  }
                }}
              />
              {showTooltip && (
                <Tooltip targetRef={tooltipRef} position="top">
                  <p className="text-sm text-center leading-relaxed">
                    {description}
                  </p>
                </Tooltip>
              )}
            </div>
          </div>
          <div className="w-full">{children}</div>
        </div>
      );
    }

    return (
      <div className={containerClasses}>
        <div className="mb-2">
          <h3
            className={`text-sm font-semibold ${disabled ? "opacity-50" : ""}`}
          >
            {title}
          </h3>
          <p className={`text-sm text-text/60 ${disabled ? "opacity-50" : ""}`}>
            {description}
          </p>
        </div>
        <div className="w-full">{children}</div>
      </div>
    );
  }

  // Horizontal layout (default)
  const horizontalContainerClasses = grouped
    ? "flex items-center justify-between gap-4 px-4 py-3 transition-colors duration-200 hover:bg-white/25 dark:hover:bg-white/5"
    : "frost-surface flex items-center justify-between gap-4 rounded-lg px-4 py-3 transition-[background-color,border-color,box-shadow,transform] duration-200 hover:border-logo-primary/20";

  if (descriptionMode === "tooltip") {
    return (
      <div className={horizontalContainerClasses}>
        <div className="max-w-[66%]">
          <div className="flex items-center gap-2">
            <h3
              className={`text-sm font-semibold ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            <div
              ref={tooltipRef}
              className="relative"
              onMouseEnter={() => setShowTooltip(true)}
              onMouseLeave={() => setShowTooltip(false)}
              onClick={toggleTooltip}
            >
              <Info
                className="h-4 w-4 cursor-help select-none text-mid-gray transition-colors duration-150 hover:text-logo-primary"
                aria-label="More information"
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggleTooltip();
                  }
                }}
              />
              {showTooltip && (
                <Tooltip targetRef={tooltipRef} position={tooltipPosition}>
                  <p className="text-sm text-center leading-relaxed">
                    {description}
                  </p>
                </Tooltip>
              )}
            </div>
          </div>
        </div>
        <div className="relative min-w-0">{children}</div>
      </div>
    );
  }

  return (
    <div className={horizontalContainerClasses}>
      <div className="max-w-[66%]">
        <h3 className={`text-sm font-semibold ${disabled ? "opacity-50" : ""}`}>
          {title}
        </h3>
        <p className={`text-sm text-text/60 ${disabled ? "opacity-50" : ""}`}>
          {description}
        </p>
      </div>
      <div className="relative min-w-0">{children}</div>
    </div>
  );
};
