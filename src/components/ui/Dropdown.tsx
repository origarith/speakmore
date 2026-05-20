import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

export interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface DropdownProps {
  options: DropdownOption[];
  className?: string;
  selectedValue: string | null;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  onRefresh?: () => void;
}

export const Dropdown: React.FC<DropdownProps> = ({
  options,
  selectedValue,
  onSelect,
  className = "",
  placeholder = "Select an option...",
  disabled = false,
  onRefresh,
}) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const selectedOption = options.find(
    (option) => option.value === selectedValue,
  );

  const handleSelect = (value: string) => {
    onSelect(value);
    setIsOpen(false);
  };

  const handleToggle = () => {
    if (disabled) return;
    if (!isOpen && onRefresh) onRefresh();
    setIsOpen(!isOpen);
  };

  return (
    <div className={`relative ${className}`} ref={dropdownRef}>
      <button
        type="button"
        className={`frost-control flex w-full min-w-0 items-center justify-between gap-2 rounded-md border px-2.5 py-1.5 text-start text-sm font-medium transition-[background-color,border-color,box-shadow] duration-150 ${
          disabled
            ? "opacity-50 cursor-not-allowed"
            : "hover:border-logo-primary/35 cursor-pointer focus:ring-2 focus:ring-logo-primary/20"
        }`}
        onClick={handleToggle}
        disabled={disabled}
        title={selectedOption?.label || placeholder}
      >
        <span className="min-w-0 flex-1 truncate">
          {selectedOption?.label || placeholder}
        </span>
        <svg
          className={`h-4 w-4 shrink-0 text-text/60 transition-transform duration-200 ${isOpen ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </button>
      {isOpen && !disabled && (
        <div className="frost-menu absolute left-0 right-0 top-full z-50 mt-1 max-h-60 overflow-y-auto rounded-md py-1">
          {options.length === 0 ? (
            <div className="px-2.5 py-1.5 text-sm text-mid-gray">
              {t("common.noOptionsFound")}
            </div>
          ) : (
            options.map((option) => (
              <button
                key={option.value}
                type="button"
                className={`w-full px-2.5 py-1.5 text-start text-sm transition-colors duration-150 hover:bg-logo-primary/10 ${
                  selectedValue === option.value
                    ? "bg-logo-primary/15 font-semibold text-logo-primary"
                    : ""
                } ${option.disabled ? "opacity-50 cursor-not-allowed" : ""}`}
                onClick={() => handleSelect(option.value)}
                disabled={option.disabled}
                title={option.label}
              >
                <span className="block min-w-0 truncate">{option.label}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
};
