import React from "react";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

export const Input: React.FC<InputProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  const baseClasses =
    "frost-control px-2 py-1 text-sm font-medium text-text placeholder:text-text/35 border rounded-md text-start transition-[background-color,border-color,box-shadow,transform] duration-200";

  const interactiveClasses = disabled
    ? "opacity-60 cursor-not-allowed"
    : "hover:border-logo-primary/35 focus:outline-none focus:border-logo-primary focus:ring-2 focus:ring-logo-primary/20";

  const variantClasses = {
    default: "px-3 py-2",
    compact: "px-2 py-1",
  } as const;

  return (
    <input
      className={`${baseClasses} ${variantClasses[variant]} ${interactiveClasses} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
