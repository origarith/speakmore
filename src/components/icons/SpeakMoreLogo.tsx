import React from "react";

interface SpeakMoreLogoProps {
  width?: number | string;
  height?: number | string;
  className?: string;
  showText?: boolean;
  brandName?: string;
  tagline?: string;
}

const SpeakMoreLogo: React.FC<SpeakMoreLogoProps> = ({
  width,
  height,
  className = "",
  showText = true,
  brandName = "SpeakMore",
  tagline,
}) => {
  return (
    <div
      className={`inline-flex min-w-0 items-center gap-2.5 ${className}`}
      style={{ width, height }}
    >
      <span
        className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-white/35 bg-logo-primary text-white shadow-[0_12px_24px_rgba(47,143,131,0.24)]"
        aria-hidden="true"
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 64 64"
          fill="currentColor"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect x="9" y="28" width="4" height="8" rx="2" />
          <rect x="16" y="25" width="4" height="14" rx="2" />
          <rect x="23" y="20" width="4" height="24" rx="2" />
          <path d="M32 10C33.1 10 34 10.9 34 12V47.8C34 48.9 33.5 49.9 32.7 50.7L30.8 53.5C30.5 54 30 53.7 30 53.2V12C30 10.9 30.9 10 32 10Z" />
          <rect x="37" y="20" width="4" height="24" rx="2" />
          <rect x="44" y="25" width="4" height="14" rx="2" />
          <rect x="51" y="28" width="4" height="8" rx="2" />
        </svg>
      </span>
      {showText && (
        <span className="min-w-0">
          <span className="block truncate text-[19px] font-semibold leading-5 text-text">
            {brandName}
          </span>
          {tagline && (
            <span className="mt-1 block truncate text-[11px] font-medium leading-3 text-text/50">
              {tagline}
            </span>
          )}
        </span>
      )}
    </div>
  );
};

export default SpeakMoreLogo;
