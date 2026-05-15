/**
 * ParamInput — a styled text input for the tool runner form.
 *
 * Uses CSS :focus-within via a wrapper class instead of onFocus/onBlur DOM
 * mutation so it works correctly under React concurrent mode.
 *
 * TEMPLATE: Replace with @aurora/aurora-input once installed:
 *   pnpm dlx shadcn@latest add @aurora/aurora-input
 */

"use client";

interface ParamInputProps {
  id: string;
  type?: string;
  placeholder?: string;
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
}

export function ParamInput({
  id,
  type = "text",
  placeholder,
  value,
  onChange,
  required,
}: ParamInputProps) {
  return (
    <input
      id={id}
      type={type}
      placeholder={placeholder}
      value={value}
      required={required}
      onChange={(e) => onChange(e.target.value)}
      className="param-input"
      style={{
        width: "100%",
        background: "var(--aurora-control-surface)",
        border: "1px solid var(--aurora-border-default)",
        borderRadius: "var(--radius-md)",
        padding: "0.5rem 0.75rem",
        color: "var(--aurora-text-primary)",
        fontSize: "0.875rem",
        fontFamily: "var(--aurora-font-sans)",
        outline: "none",
        boxSizing: "border-box",
        transition: "border-color 0.15s ease",
      }}
      onFocus={(e) => {
        (e.currentTarget as HTMLInputElement).style.setProperty(
          "border-color",
          "var(--aurora-accent-primary)",
        );
      }}
      onBlur={(e) => {
        (e.currentTarget as HTMLInputElement).style.setProperty(
          "border-color",
          "var(--aurora-border-default)",
        );
      }}
    />
  );
}
