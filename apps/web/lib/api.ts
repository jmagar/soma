/**
 * Typed client for the rmcp-template REST API.
 *
 * All actions are dispatched via POST /v1/example with:
 *   { "action": "<action>", "params": { ... } }
 *
 * The base URL is relative (empty string) so the same binary serves
 * both the API and the web UI — no CORS or cross-origin config needed.
 */

const BASE = "";

export interface ApiResponse<T = unknown> {
  data?: T;
  error?: string;
}

export interface GreetResult {
  greeting: string;
  target: string;
  server?: string;
}

export interface EchoResult {
  echo: string;
}

export interface StatusResult {
  status: string;
  api_url?: string;
  note?: string;
  [key: string]: unknown;
}

export interface HealthResult {
  status: string;
}

/** POST /v1/example — dispatch an action */
export async function callAction<T = unknown>(
  action: string,
  params: Record<string, unknown> = {}
): Promise<ApiResponse<T>> {
  try {
    const res = await fetch(`${BASE}/v1/example`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action, params }),
    });
    const json = await res.json();
    if (!res.ok) {
      return { error: json.error ?? `HTTP ${res.status}` };
    }
    return { data: json as T };
  } catch (e) {
    return { error: e instanceof Error ? e.message : "Network error" };
  }
}

/** GET /health */
export async function getHealth(): Promise<ApiResponse<HealthResult>> {
  try {
    const res = await fetch(`${BASE}/health`);
    const json = await res.json();
    if (!res.ok) return { error: `HTTP ${res.status}` };
    return { data: json as HealthResult };
  } catch (e) {
    return { error: e instanceof Error ? e.message : "Network error" };
  }
}

/** GET /status */
export async function getStatus(): Promise<ApiResponse<StatusResult>> {
  try {
    const res = await fetch(`${BASE}/status`);
    const json = await res.json();
    if (!res.ok) return { error: `HTTP ${res.status}` };
    return { data: json as StatusResult };
  } catch (e) {
    return { error: e instanceof Error ? e.message : "Network error" };
  }
}

export const greet = (name?: string) =>
  callAction<GreetResult>("greet", name ? { name } : {});

export const echo = (message: string) =>
  callAction<EchoResult>("echo", { message });

export const status = () =>
  callAction<StatusResult>("status");
