export function ActionButton({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        background: "var(--aurora-accent-button)",
        color: "var(--aurora-accent-foreground)",
        border: "none",
        borderRadius: "var(--radius-md)",
        padding: "0.5rem 1rem",
        fontSize: "0.875rem",
        fontWeight: 600,
        cursor: "pointer",
      }}
      onMouseEnter={(e) => {
        (e.target as HTMLElement).style.background = "var(--aurora-accent-primary)";
      }}
      onMouseLeave={(e) => {
        (e.target as HTMLElement).style.background = "var(--aurora-accent-button)";
      }}
    >
      {label}
    </button>
  );
}
