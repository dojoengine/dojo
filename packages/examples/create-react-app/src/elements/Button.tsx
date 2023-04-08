import React from "react";

interface ButtonProps {
  onClick: () => void;
  children: React.ReactNode;
  className?: string;
  disabled?: boolean;
}

const Button: React.FC<ButtonProps> = ({
  onClick,
  children,
  className = "",
  disabled = false,
}) => {
  const baseStyle =
    "inline-flex items-center justify-center px-4 py-2 text-base font-medium text-white border border-transparent rounded-md shadow-sm";
  const enabledStyle =
    "bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500";
  const disabledStyle = "bg-gray-300 cursor-not-allowed";

  return (
    <button
      type="button"
      onClick={disabled ? undefined : onClick}
      className={`${baseStyle} ${
        disabled ? disabledStyle : enabledStyle
      } ${className}`}
      disabled={disabled}
    >
      {children}
    </button>
  );
};

export default Button;
