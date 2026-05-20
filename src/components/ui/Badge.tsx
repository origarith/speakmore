import React from "react";

interface BadgeProps {
  children: React.ReactNode;
  variant?: "primary" | "success" | "secondary" | "warning" | "danger";
  className?: string;
}

const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "primary",
  className = "",
}) => {
  const variantClasses = {
    primary: "bg-logo-primary/10 text-logo-primary ring-1 ring-logo-primary/20",
    success: "bg-success/10 text-success ring-1 ring-success/20",
    secondary:
      "bg-white/50 text-text/70 ring-1 ring-frost-border dark:bg-white/10",
    warning: "bg-warning/10 text-warning ring-1 ring-warning/20",
    danger: "bg-danger/10 text-danger ring-1 ring-danger/20",
  };

  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-semibold leading-none ${variantClasses[variant]} ${className}`}
    >
      {children}
    </span>
  );
};

export default Badge;
