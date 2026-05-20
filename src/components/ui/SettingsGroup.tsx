import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
  className?: string;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
  className = "",
}) => {
  return (
    <section className={`space-y-2 ${className}`}>
      {title && (
        <div className="px-4">
          <h2 className="text-[11px] font-semibold uppercase text-text/50">
            {title}
          </h2>
          {description && (
            <p className="mt-1 text-xs text-text/50">{description}</p>
          )}
        </div>
      )}
      <div className="frost-surface overflow-visible rounded-lg">
        <div className="divide-y divide-frost-border">{children}</div>
      </div>
    </section>
  );
};
