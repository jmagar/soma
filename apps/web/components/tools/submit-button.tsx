/**
 * SubmitButton — a styled form submit button for the tool runner.
 *
 * TEMPLATE: Replace with @aurora/aurora-button once installed:
 *   pnpm dlx shadcn@latest add @aurora/aurora-button
 */

"use client";

interface SubmitButtonProps {
  loading: boolean;
  label?: string;
  loadingLabel?: string;
}

export function SubmitButton({
  loading,
  label = "Run Action",
  loadingLabel = "Running…",
}: SubmitButtonProps) {
  return (
    <button
      type="submit"
      disabled={loading}
      style={{
        background: loading
          ? "var(--aurora-panel-strong)"
          : "var(--aurora-accent-button)",
        color: loading
          ? "var(--aurora-text-muted)"
          : "var(--aurora-accent-foreground)",
        border: "none",
        borderRadius: "var(--radius-md)",
        padding: "0.5rem 1.25rem",
        fontWeight: 600,
        fontSize: "0.875rem",
        cursor: loading ? "not-allowed" : "pointer",
      }}
    >
      {loading ? loadingLabel : label}
    </button>
  );
}
