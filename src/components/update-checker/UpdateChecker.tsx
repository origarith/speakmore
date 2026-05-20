import React from "react";
import { useTranslation } from "react-i18next";

interface UpdateCheckerProps {
  className?: string;
}

const UpdateChecker: React.FC<UpdateCheckerProps> = ({ className = "" }) => {
  const { t } = useTranslation();

  return (
    <span className={`text-text/60 tabular-nums ${className}`}>
      {t("footer.updateCheckingDisabled")}
    </span>
  );
};

export default UpdateChecker;
