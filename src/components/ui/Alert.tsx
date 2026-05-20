import React from "react";
import { AlertCircle, AlertTriangle, Info, CheckCircle } from "lucide-react";

type AlertVariant = "error" | "warning" | "info" | "success";

interface AlertProps {
  variant?: AlertVariant;
  /** When true, removes rounded corners for use inside containers */
  contained?: boolean;
  children: React.ReactNode;
  className?: string;
}

const variantStyles: Record<
  AlertVariant,
  { container: string; icon: string; text: string }
> = {
  error: {
    container: "bg-danger/10 border-danger/20",
    icon: "text-danger",
    text: "text-danger",
  },
  warning: {
    container: "bg-warning/10 border-warning/20",
    icon: "text-warning",
    text: "text-warning",
  },
  info: {
    container: "bg-logo-primary/10 border-logo-primary/20",
    icon: "text-logo-primary",
    text: "text-text/70",
  },
  success: {
    container: "bg-success/10 border-success/20",
    icon: "text-success",
    text: "text-success",
  },
};

const variantIcons: Record<AlertVariant, React.ElementType> = {
  error: AlertCircle,
  warning: AlertTriangle,
  info: Info,
  success: CheckCircle,
};

export const Alert: React.FC<AlertProps> = ({
  variant = "error",
  contained = false,
  children,
  className = "",
}) => {
  const styles = variantStyles[variant];
  const Icon = variantIcons[variant];

  return (
    <div
      className={`flex items-start gap-3 border p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.28)] ${styles.container} ${contained ? "" : "rounded-lg"} ${className}`}
    >
      <Icon className={`w-5 h-5 shrink-0 mt-0.5 ${styles.icon}`} />
      <p className={`text-sm ${styles.text}`}>{children}</p>
    </div>
  );
};
