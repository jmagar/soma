/**
 * Typed client for the rmcp-template REST API.
 *
 * All actions are dispatched via POST /v1/example with:
 *   { "action": "<action>", "params": { ... } }
 *
 * The base URL is relative (empty string) so the same binary serves
 * both the API and the web UI — no CORS or cross-origin config needed.
 */

import { endpoint, WEB_APP_CONFIG } from "@/lib/template";

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
}

export interface HealthResult {
  status: string;
}

/** Shared fetch helper — handles JSON parsing and error normalisation. */
async function apiFetch<T>(url: string, options?: RequestInit): Promise<ApiResponse<T>> {
  try {
    const res = await fetch(url, options);
    const json = await res.json();
    if (!res.ok) {
      return { error: (json as { error?: string }).error ?? `HTTP ${res.status}` };
    }
    return { data: json as T };
  } catch (e) {
    return { error: e instanceof Error ? e.message : "Network error" };
  }
}

/** POST /v1/example — dispatch an action */
export function callAction<T = unknown>(
  action: string,
  params: Record<string, unknown> = {},
): Promise<ApiResponse<T>> {
  return apiFetch<T>(endpoint(WEB_APP_CONFIG.restEndpoint), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action, params }),
  });
}

/** GET /health */
export function getHealth(): Promise<ApiResponse<HealthResult>> {
  return apiFetch<HealthResult>(endpoint(WEB_APP_CONFIG.healthEndpoint));
}

/** GET /status */
export function getStatus(): Promise<ApiResponse<StatusResult>> {
  return apiFetch<StatusResult>(endpoint(WEB_APP_CONFIG.statusEndpoint));
}

export const greet = (name?: string) => callAction<GreetResult>("greet", name ? { name } : {});

export const echo = (message: string) => callAction<EchoResult>("echo", { message });

export const status = () => callAction<StatusResult>("status");
