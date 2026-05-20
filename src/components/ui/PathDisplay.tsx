import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "./Button";

interface PathDisplayProps {
  path: string;
  onOpen: () => void;
  disabled?: boolean;
  wrap?: boolean;
  className?: string;
}

export const PathDisplay: React.FC<PathDisplayProps> = ({
  path,
  onOpen,
  disabled = false,
  wrap = true,
  className = "",
}) => {
  const { t } = useTranslation();

  return (
    <div className={`flex min-w-0 items-center gap-2 ${className}`}>
      <div
        className={`frost-control min-w-0 flex-1 cursor-text select-text rounded-md border px-2 py-2 font-mono text-xs ${wrap ? "break-all" : "overflow-x-auto whitespace-nowrap"}`}
        title={path}
      >
        {path}
      </div>
      <Button
        onClick={onOpen}
        variant="secondary"
        size="sm"
        disabled={disabled}
        className="px-3 py-2"
      >
        {t("common.open")}
      </Button>
    </div>
  );
};
