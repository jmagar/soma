"use client";

import { useState } from "react";
import { callAction } from "@/lib/api";

const ACTIONS = [
  {
    id: "greet",
    label: "greet",
    description: "Return a personalised greeting",
    params: [{ name: "name", label: "Name", type: "text", placeholder: "Alice", required: false }],
  },
  {
    id: "echo",
    label: "echo",
    description: "Echo a message back unchanged",
    params: [{ name: "message", label: "Message", type: "text", placeholder: "Hello!", required: true }],
  },
  {
    id: "status",
    label: "status",
    description: "Return server status and configuration",
    params: [],
  },
  {
    id: "help",
    label: "help",
    description: "Show all available actions",
    params: [],
  },
] as const;

type ActionId = (typeof ACTIONS)[number]["id"];

export default function ToolsPage() {
  const [selectedAction, setSelectedAction] = useState<ActionId>("greet");
  const [paramValues, setParamValues] = useState<Record<string, string>>({});
  const [response, setResponse] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [isError, setIsError] = useState(false);

  const action = ACTIONS.find((a) => a.id === selectedAction)!;

  const handleSelect = (id: ActionId) => {
    setSelectedAction(id);
    setParamValues({});
    setResponse(null);
    setIsError(false);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setResponse(null);
    setIsError(false);

    // Build params — only include non-empty values
    const params: Record<string, string> = {};
    for (const [k, v] of Object.entries(paramValues)) {
      if (v.trim()) params[k] = v.trim();
    }

    const res = await callAction(selectedAction, params);
    setLoading(false);

    if (res.error) {
      setResponse(JSON.stringify({ error: res.error }, null, 2));
      setIsError(true);
    } else {
      setResponse(JSON.stringify(res.data, null, 2));
    }
  };

  return (
    <div className="max-w-4xl mx-auto space-y-6">
      {/* Header */}
      <div>
        <h1
          style={{
            fontFamily: "var(--aurora-font-display)",
            fontSize: "1.75rem",
            fontWeight: 700,
            marginBottom: "0.25rem",
          }}
        >
          Tool Runner
        </h1>
        <p style={{ color: "var(--aurora-text-muted)", fontSize: "0.875rem" }}>
          Call any action via{" "}
          <code
            style={{
              fontFamily: "var(--aurora-font-mono)",
              background: "var(--aurora-panel-strong)",
              padding: "0.1em 0.4em",
              borderRadius: "4px",
              fontSize: "0.8em",
            }}
          >
            POST /v1/example
          </code>
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Action selector */}
        <div
          style={{
            background: "var(--aurora-panel-medium)",
            border: "1px solid var(--aurora-border-default)",
            borderRadius: "var(--radius-lg)",
            padding: "1rem",
          }}
        >
          <p
            style={{
              color: "var(--aurora-text-muted)",
              fontSize: "0.75rem",
              fontWeight: 600,
              textTransform: "uppercase",
              letterSpacing: "0.05em",
              marginBottom: "0.75rem",
            }}
          >
            Actions
          </p>
          <div className="space-y-1">
            {ACTIONS.map((a) => (
              <button
                key={a.id}
                onClick={() => handleSelect(a.id)}
                style={{
                  width: "100%",
                  textAlign: "left",
                  padding: "0.5rem 0.75rem",
                  borderRadius: "var(--radius-md)",
                  border: "none",
                  cursor: "pointer",
                  fontSize: "0.875rem",
                  fontFamily: "var(--aurora-font-mono)",
                  background:
                    selectedAction === a.id
                      ? "var(--aurora-hover-bg)"
                      : "transparent",
                  color:
                    selectedAction === a.id
                      ? "var(--aurora-accent-primary)"
                      : "var(--aurora-text-primary)",
                  borderLeft:
                    selectedAction === a.id
                      ? "2px solid var(--aurora-accent-primary)"
                      : "2px solid transparent",
                }}
              >
                {a.label}
              </button>
            ))}
          </div>
        </div>

        {/* Form + response */}
        <div className="md:col-span-2 space-y-4">
          {/* Param form */}
          <form
            onSubmit={handleSubmit}
            style={{
              background: "var(--aurora-panel-medium)",
              border: "1px solid var(--aurora-border-default)",
              borderRadius: "var(--radius-lg)",
              padding: "1.25rem",
            }}
          >
            <p
              style={{
                color: "var(--aurora-text-muted)",
                fontSize: "0.75rem",
                fontWeight: 600,
                textTransform: "uppercase",
                letterSpacing: "0.05em",
                marginBottom: "0.5rem",
              }}
            >
              {action.label}
            </p>
            <p style={{ color: "var(--aurora-text-muted)", fontSize: "0.8rem", marginBottom: "1rem" }}>
              {action.description}
            </p>

            {action.params.length > 0 ? (
              <div className="space-y-3 mb-4">
                {action.params.map((param) => (
                  <div key={param.name}>
                    <label
                      htmlFor={param.name}
                      style={{
                        display: "block",
                        color: "var(--aurora-text-muted)",
                        fontSize: "0.75rem",
                        marginBottom: "0.25rem",
                        fontWeight: 500,
                      }}
                    >
                      {param.label}
                      {param.required && (
                        <span style={{ color: "var(--aurora-error)", marginLeft: "0.25rem" }}>*</span>
                      )}
                    </label>
                    <input
                      id={param.name}
                      type={param.type}
                      placeholder={param.placeholder}
                      value={paramValues[param.name] ?? ""}
                      onChange={(e) =>
                        setParamValues((prev) => ({ ...prev, [param.name]: e.target.value }))
                      }
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
                      }}
                      onFocus={(e) => {
                        e.target.style.borderColor = "var(--aurora-accent-primary)";
                      }}
                      onBlur={(e) => {
                        e.target.style.borderColor = "var(--aurora-border-default)";
                      }}
                    />
                  </div>
                ))}
              </div>
            ) : (
              <p style={{ color: "var(--aurora-text-muted)", fontSize: "0.8rem", marginBottom: "1rem" }}>
                No parameters required.
              </p>
            )}

            <button
              type="submit"
              disabled={loading}
              style={{
                background: loading ? "var(--aurora-panel-strong)" : "var(--aurora-accent-button)",
                color: loading ? "var(--aurora-text-muted)" : "var(--aurora-accent-foreground)",
                border: "none",
                borderRadius: "var(--radius-md)",
                padding: "0.5rem 1.25rem",
                fontWeight: 600,
                fontSize: "0.875rem",
                cursor: loading ? "not-allowed" : "pointer",
              }}
            >
              {loading ? "Running…" : "Run Action"}
            </button>
          </form>

          {/* Response */}
          {response !== null && (
            <div
              style={{
                background: "var(--aurora-panel-strong)",
                border: `1px solid ${isError ? "var(--aurora-error)" : "var(--aurora-border-default)"}`,
                borderRadius: "var(--radius-lg)",
                padding: "1.25rem",
              }}
            >
              <p
                style={{
                  color: isError ? "var(--aurora-error)" : "var(--aurora-text-muted)",
                  fontSize: "0.75rem",
                  fontWeight: 600,
                  textTransform: "uppercase",
                  letterSpacing: "0.05em",
                  marginBottom: "0.75rem",
                }}
              >
                {isError ? "Error" : "Response"}
              </p>
              <pre
                style={{
                  color: isError ? "var(--aurora-error)" : "var(--aurora-accent-strong)",
                  fontFamily: "var(--aurora-font-mono)",
                  fontSize: "0.8rem",
                  overflow: "auto",
                  margin: 0,
                  whiteSpace: "pre-wrap",
                  wordBreak: "break-word",
                }}
              >
                {response}
              </pre>
            </div>
          )}

          {/* Request preview */}
          <div
            style={{
              background: "var(--aurora-panel-medium)",
              border: "1px solid var(--aurora-border-default)",
              borderRadius: "var(--radius-lg)",
              padding: "1rem",
            }}
          >
            <p
              style={{
                color: "var(--aurora-text-muted)",
                fontSize: "0.75rem",
                fontWeight: 600,
                textTransform: "uppercase",
                letterSpacing: "0.05em",
                marginBottom: "0.5rem",
              }}
            >
              Request Preview
            </p>
            <pre
              style={{
                color: "var(--aurora-text-muted)",
                fontFamily: "var(--aurora-font-mono)",
                fontSize: "0.75rem",
                margin: 0,
                whiteSpace: "pre-wrap",
              }}
            >
              {`POST /v1/example\nContent-Type: application/json\n\n${JSON.stringify(
                {
                  action: selectedAction,
                  params: Object.fromEntries(
                    Object.entries(paramValues).filter(([, v]) => v.trim())
                  ),
                },
                null,
                2
              )}`}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}
