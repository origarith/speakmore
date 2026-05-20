import React from "react";
import { Input } from "../../ui/Input";

interface ApiKeyFieldProps {
  value: string;
  onChange: (value: string) => void;
  onBlur: () => void;
  disabled: boolean;
  placeholder?: string;
  className?: string;
}

export const ApiKeyField: React.FC<ApiKeyFieldProps> = React.memo(
  ({ value, onChange, onBlur, disabled, placeholder, className = "" }) => {
    return (
      <Input
        type="password"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onBlur={onBlur}
        placeholder={placeholder}
        variant="compact"
        disabled={disabled}
        className={`w-full min-w-0 ${className}`}
      />
    );
  },
);

ApiKeyField.displayName = "ApiKeyField";
