import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "primary-soft"
    | "secondary"
    | "danger"
    | "danger-ghost"
    | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button: React.FC<ButtonProps> = ({
  children,
  className = "",
  variant = "primary",
  size = "md",
  ...props
}) => {
  const baseClasses =
    "min-h-8 font-medium rounded-md border focus:outline-none transition-[background-color,border-color,box-shadow,transform,color] duration-200 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer active:scale-[0.98]";

  const variantClasses = {
    primary:
      "text-white bg-background-ui border-background-ui shadow-[0_10px_22px_rgba(47,143,131,0.22)] hover:bg-logo-primary hover:border-logo-primary hover:shadow-[0_12px_26px_rgba(47,143,131,0.28)] focus:ring-2 focus:ring-logo-primary/30",
    "primary-soft":
      "text-logo-primary bg-logo-primary/10 border-logo-primary/20 hover:bg-logo-primary/15 hover:border-logo-primary/30 focus:ring-2 focus:ring-logo-primary/25",
    secondary:
      "frost-control text-text hover:border-logo-primary/35 hover:text-logo-primary focus:ring-2 focus:ring-logo-primary/20",
    danger:
      "text-white bg-danger border-danger shadow-sm hover:bg-danger/90 hover:border-danger/90 focus:ring-2 focus:ring-danger/30",
    "danger-ghost":
      "text-danger border-transparent hover:bg-danger/10 focus:bg-danger/15",
    ghost:
      "text-current border-transparent hover:bg-white/55 hover:border-frost-border focus:bg-white/55 dark:hover:bg-white/10 dark:focus:bg-white/10",
  };

  const sizeClasses = {
    sm: "px-2.5 py-1 text-xs",
    md: "px-4 py-1.5 text-sm",
    lg: "px-5 py-2 text-base",
  };

  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
};
