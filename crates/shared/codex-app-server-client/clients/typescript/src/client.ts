// Thin, hand-written runtime wrapper around the generated `openapi-types.ts`
// types. This file is NOT generated - see README.md's "Why a thin
// hand-written wrapper, not a full generated client" section. It has zero
// runtime dependencies: only the platform `fetch`/`URL`/`TextDecoder`
// globals, available in every modern browser and in Node.js >=18.
//
// One method per route in ../../openapi.json's `paths` (13 routes across 12
// path items - see that file's top-level comment in README.md). Every method
// name below is cross-referenced to its OpenAPI `operationId` in a comment.

import type { components } from "./generated/openapi-types.ts";

type Schemas = components["schemas"];

export type RestHealthResponse = Schemas["RestHealthResponse"];
export type CompatibilityReport = Schemas["CompatibilityReport"];
export type RestTextTurnRequest = Schemas["RestTextTurnRequest"];
export type RestTextTurnResponse = Schemas["RestTextTurnResponse"];
export type RestCallBody = Schemas["RestCallBody"];
export type RestCallResponse = Schemas["RestCallResponse"];
export type RestClientOptions = Schemas["RestClientOptions"];
export type RestSessionCreateRequest = Schemas["RestSessionCreateRequest"];
export type RestSessionCreateResponse = Schemas["RestSessionCreateResponse"];
export type RestListSessionsResponse = Schemas["RestListSessionsResponse"];
export type RestSessionSummary = Schemas["RestSessionSummary"];
export type RestStatusResponse = Schemas["RestStatusResponse"];
export type RestEventResponse = Schemas["RestEventResponse"];
export type RestErrorResponse = Schemas["RestErrorResponse"];
export type RestErrorReplyRequest = Schemas["RestErrorReplyRequest"];
export type RestRequestReplyResultRequest = Schemas["RestRequestReplyResultRequest"];
export type RestRequestReplyResponse = Schemas["RestRequestReplyResponse"];

/**
 * Thrown for every non-2xx HTTP response this client makes, including the
 * one case the SSE stream route can raise mid-stream (see
 * `CodexAppServerRestClient.streamEvents`'s doc comment) - `status` is `null`
 * there since no HTTP status line can follow a committed `200`
 * `text/event-stream` response, exactly as `openapi.json`'s
 * `getV1SessionsBySessionIdEventsStream` operation description documents.
 *
 * Deliberately no constructor parameter properties here (`constructor(public
 * readonly status: ...)`) - that TypeScript syntax lowers to real runtime
 * assignment code, which Node's built-in "erasable syntax only" TypeScript
 * support (the mode `examples/smoke.ts` and this package's own scripts run
 * under with plain `node`, no `--experimental-transform-types`/build step)
 * rejects with `ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX`. Explicit field
 * declarations plus a manual assignment in the constructor body are the
 * erasable-syntax-safe equivalent - see README.md's "Running TypeScript
 * directly with `node`" section.
 */
export class CodexAppServerRestError extends Error {
  readonly status: number | null;
  readonly body: RestErrorResponse;

  constructor(status: number | null, body: RestErrorResponse) {
    super(
      `codex-app-server-rest: ${status === null ? "stream" : `HTTP ${status}`}: ` +
        `${body.error}: ${body.message}`,
    );
    this.name = "CodexAppServerRestError";
    this.status = status;
    this.body = body;
  }
}

export interface CodexAppServerRestClientOptions {
  /** e.g. `http://127.0.0.1:43210`. No trailing slash required. */
  baseUrl: string;
  /** Sent as `Authorization: Bearer <token>` when the server is wrapped in `rest::bearer_auth(...)`. */
  token?: string;
  /** Override for tests or non-global-fetch runtimes. Defaults to the ambient `fetch`. */
  fetch?: typeof fetch;
}

/**
 * Joins a JSON-RPC method name (e.g. `"thread/start"`) onto a REST path
 * segment without collapsing it into a single percent-encoded token.
 *
 * `openapi.json`'s `method` path parameter description (see `/v1/call/{method}`
 * and `/v1/sessions/{sessionId}/call/{method}`) documents that the server
 * captures this via an axum `{*method}` wildcard: the literal `/` characters
 * in a method name are structurally part of the URL path, not something a
 * spec-honest client should escape into `%2F`. This was verified against a
 * live `codex-app-server-rest --mode trusted-bridge` instance: both
 * `/v1/call/thread/start` and the naively-escaped `/v1/call/thread%2Fstart`
 * reached the exact same handler and produced identical responses (axum/hyper
 * decode `%2F` back to `/` before wildcard matching - see
 * clients/typescript/README.md's "The `{method}` wildcard" section for the
 * live transcript). This function still only encodes special characters
 * *within* each `/`-delimited segment and leaves `/` itself unescaped, since
 * that's the form `openapi.json` documents as the real shape of this
 * parameter, and it is not dependent on axum's specific percent-decode
 * behavior the way a raw `encodeURIComponent(method)` would be.
 */
export function encodeMethodPath(method: string): string {
  const trimmed = method.replace(/^\/+|\/+$/g, "");
  if (trimmed.length === 0) {
    throw new TypeError("method must not be empty");
  }
  return trimmed
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

/** One parsed Server-Sent Events frame (an `event:`/`data:` pair, `data:` may span multiple lines). */
interface SseFrame {
  event?: string;
  data: string;
}

/**
 * Parses a `text/event-stream` body into whole frames. Hand-rolled (no
 * `eventsource-parser`/`EventSource` dependency) because the client stays at
 * zero runtime dependencies - see README.md. Handles exactly what this
 * server emits (`event:` + one-or-more `data:` lines per frame, blank-line
 * delimited, `:`-prefixed comment/keep-alive lines ignored) rather than the
 * full SSE spec (no `id:`/`retry:` support, since this server never sends
 * them - see `openapi.json`'s SSE operation description).
 */
async function* parseSseStream(body: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      buffer += decoder.decode(value, { stream: true });
      let boundary = buffer.indexOf("\n\n");
      while (boundary !== -1) {
        const frame = parseSseFrame(buffer.slice(0, boundary));
        if (frame) {
          yield frame;
        }
        buffer = buffer.slice(boundary + 2);
        boundary = buffer.indexOf("\n\n");
      }
    }
  } finally {
    reader.releaseLock();
  }
}

function parseSseFrame(raw: string): SseFrame | null {
  let event: string | undefined;
  const dataLines: string[] = [];
  for (const line of raw.split("\n")) {
    if (line.length === 0 || line.startsWith(":")) {
      continue;
    }
    if (line.startsWith("event:")) {
      event = line.slice("event:".length).trim();
    } else if (line.startsWith("data:")) {
      dataLines.push(line.slice("data:".length).trim());
    }
  }
  if (dataLines.length === 0) {
    return null;
  }
  const data = dataLines.join("\n");
  return event === undefined ? { data } : { event, data };
}

/**
 * Thin fetch-based client for every route in `openapi.json`. One public
 * method per `operationId`; each method's doc comment names it.
 */
export class CodexAppServerRestClient {
  private readonly baseUrl: string;
  private readonly token: string | undefined;
  private readonly fetchImpl: typeof fetch;

  constructor(options: CodexAppServerRestClientOptions) {
    this.baseUrl = options.baseUrl.endsWith("/") ? options.baseUrl : `${options.baseUrl}/`;
    this.token = options.token;
    this.fetchImpl = options.fetch ?? globalThis.fetch.bind(globalThis);
  }

  /** `GET /health` (`operationId: get_health`). Never requires auth. */
  health(): Promise<RestHealthResponse> {
    return this.requestJson<RestHealthResponse>("GET", "health");
  }

  /** `GET /v1/health` (`operationId: get_v1_health`). Never requires auth. */
  healthV1(): Promise<RestHealthResponse> {
    return this.requestJson<RestHealthResponse>("GET", "v1/health");
  }

  /** `GET /v1/compatibility` (`operationId: getV1Compatibility`). */
  compatibility(): Promise<CompatibilityReport> {
    return this.requestJson<CompatibilityReport>("GET", "v1/compatibility");
  }

  /**
   * `POST /v1/text-turn` (`operationId: postV1TextTurn`). Only mounted by
   * `text-turn`/`trusted-bridge` server modes - see `RestRouterOptions::enable_text_turn_route`.
   */
  textTurn(request: RestTextTurnRequest): Promise<RestTextTurnResponse> {
    return this.requestJson<RestTextTurnResponse>("POST", "v1/text-turn", request);
  }

  /**
   * `POST /v1/call/{method}` (`operationId: postV1CallMethod`). One-shot raw
   * JSON-RPC bridge call. Only mounted in `trusted-bridge` server mode. See
   * {@link encodeMethodPath} for how `method` is placed in the URL.
   */
  call(method: string, body: RestCallBody = {}): Promise<RestCallResponse> {
    return this.requestJson<RestCallResponse>("POST", `v1/call/${encodeMethodPath(method)}`, body);
  }

  /** `POST /v1/sessions` (`operationId: postV1Sessions`). Only mounted in `trusted-bridge` server mode. */
  createSession(body: RestSessionCreateRequest = {}): Promise<RestSessionCreateResponse> {
    return this.requestJson<RestSessionCreateResponse>("POST", "v1/sessions", body);
  }

  /** `GET /v1/sessions` (`operationId: getV1Sessions`). Only mounted in `trusted-bridge` server mode. */
  listSessions(): Promise<RestListSessionsResponse> {
    return this.requestJson<RestListSessionsResponse>("GET", "v1/sessions");
  }

  /** `DELETE /v1/sessions/{sessionId}` (`operationId: deleteV1SessionsBySessionId`). */
  deleteSession(sessionId: string): Promise<RestStatusResponse> {
    return this.requestJson<RestStatusResponse>("DELETE", `v1/sessions/${encodeURIComponent(sessionId)}`);
  }

  /**
   * `POST /v1/sessions/{sessionId}/call/{method}` (`operationId: postV1SessionsBySessionIdCallMethod`).
   * `body.client` must be omitted - the server rejects it with `400` on this route (client
   * overrides only apply at session creation or on the one-shot `call()` route above).
   */
  sessionCall(
    sessionId: string,
    method: string,
    body: Omit<RestCallBody, "client"> = {},
  ): Promise<RestCallResponse> {
    return this.requestJson<RestCallResponse>(
      "POST",
      `v1/sessions/${encodeURIComponent(sessionId)}/call/${encodeMethodPath(method)}`,
      body,
    );
  }

  /**
   * `GET /v1/sessions/{sessionId}/events` (`operationId: getV1SessionsBySessionIdEvents`).
   * Long-polls once; resolves with `{"event": "timeout"}` (not a rejection) if nothing
   * arrived within `timeoutMs`.
   */
  pollEvents(sessionId: string, timeoutMs?: number): Promise<RestEventResponse> {
    const query = timeoutMs === undefined ? undefined : { timeoutMs: String(timeoutMs) };
    return this.requestJson<RestEventResponse>(
      "GET",
      `v1/sessions/${encodeURIComponent(sessionId)}/events`,
      undefined,
      query,
    );
  }

  /**
   * `GET /v1/sessions/{sessionId}/events/stream` (`operationId: getV1SessionsBySessionIdEventsStream`),
   * the SSE counterpart to {@link pollEvents}. Yields one `RestEventResponse` per SSE frame
   * (including `timeout` heartbeats - callers that only care about real events should skip
   * those) until a `closed` event, the stream ends, or a terminal `event: error` frame is
   * received (surfaced as a thrown {@link CodexAppServerRestError} with `status: null` -
   * see that class's doc comment).
   */
  async *streamEvents(sessionId: string, timeoutMs?: number): AsyncGenerator<RestEventResponse> {
    const query = timeoutMs === undefined ? undefined : { timeoutMs: String(timeoutMs) };
    const response = await this.fetchImpl(
      this.buildUrl(`v1/sessions/${encodeURIComponent(sessionId)}/events/stream`, query),
      { headers: this.authHeaders() },
    );
    if (!response.ok || response.body === null) {
      throw new CodexAppServerRestError(response.status, await this.parseErrorBody(response));
    }
    for await (const frame of parseSseStream(response.body)) {
      if (frame.event === "error") {
        throw new CodexAppServerRestError(null, JSON.parse(frame.data) as RestErrorResponse);
      }
      const event = JSON.parse(frame.data) as RestEventResponse;
      yield event;
      if (event.event === "closed") {
        return;
      }
    }
  }

  /**
   * `POST /v1/sessions/{sessionId}/requests/{requestKey}/result`
   * (`operationId: postV1SessionsBySessionIdRequestsByRequestKeyResult`).
   */
  replyResult(
    sessionId: string,
    requestKey: string,
    body: RestRequestReplyResultRequest,
  ): Promise<RestRequestReplyResponse> {
    return this.requestJson<RestRequestReplyResponse>(
      "POST",
      `v1/sessions/${encodeURIComponent(sessionId)}/requests/${encodeURIComponent(requestKey)}/result`,
      body,
    );
  }

  /**
   * `POST /v1/sessions/{sessionId}/requests/{requestKey}/error`
   * (`operationId: postV1SessionsBySessionIdRequestsByRequestKeyError`).
   */
  replyError(
    sessionId: string,
    requestKey: string,
    body: RestErrorReplyRequest,
  ): Promise<RestRequestReplyResponse> {
    return this.requestJson<RestRequestReplyResponse>(
      "POST",
      `v1/sessions/${encodeURIComponent(sessionId)}/requests/${encodeURIComponent(requestKey)}/error`,
      body,
    );
  }

  private authHeaders(): Record<string, string> {
    return this.token === undefined ? {} : { authorization: `Bearer ${this.token}` };
  }

  private buildUrl(pathSegment: string, query?: Record<string, string>): string {
    const url = new URL(pathSegment, this.baseUrl);
    if (query) {
      for (const [key, value] of Object.entries(query)) {
        url.searchParams.set(key, value);
      }
    }
    return url.toString();
  }

  private async parseErrorBody(response: Response): Promise<RestErrorResponse> {
    const text = await response.text();
    try {
      return JSON.parse(text) as RestErrorResponse;
    } catch {
      return { error: "non_json_response", message: text || response.statusText };
    }
  }

  private async requestJson<T>(
    method: string,
    pathSegment: string,
    body?: unknown,
    query?: Record<string, string>,
  ): Promise<T> {
    const headers: Record<string, string> = { ...this.authHeaders() };
    const init: RequestInit = { method, headers };
    if (body !== undefined) {
      headers["content-type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    const response = await this.fetchImpl(this.buildUrl(pathSegment, query), init);
    const text = await response.text();
    if (!response.ok) {
      let errorBody: RestErrorResponse;
      try {
        errorBody = JSON.parse(text) as RestErrorResponse;
      } catch {
        errorBody = { error: "non_json_response", message: text || response.statusText };
      }
      throw new CodexAppServerRestError(response.status, errorBody);
    }
    return text.length === 0 ? (undefined as T) : (JSON.parse(text) as T);
  }
}
