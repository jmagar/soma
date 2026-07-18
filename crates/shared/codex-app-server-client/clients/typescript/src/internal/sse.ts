// Internal Server-Sent Events (`text/event-stream`) frame parser backing
// `CodexAppServerRestClient.streamEvents` in `../client.ts`. Split out into
// its own module (rather than living as unexported functions inside
// client.ts) so it can be unit-tested directly against synthetic
// `ReadableStream`s - byte-exact chunk boundaries, mid-character UTF-8
// splits, and truncated-at-EOF streams are all much easier to construct here
// than through a real `fetch()`/HTTP round trip. `client.ts` imports
// `parseSseStream` for its own use and re-exports only
// `CodexAppServerRestStreamTruncatedError` (the one type callers of
// `streamEvents` need to be able to `instanceof`-check) - the frame-parsing
// internals below stay unexported from the package's public API, same as
// `find-binary.ts` and `free-port.ts` in this directory.
//
// Hand-rolled (no `eventsource-parser`/`EventSource` dependency) because the
// client stays at zero runtime dependencies - see README.md. Handles exactly
// what this server emits (`event:` + one-or-more `data:` lines per frame,
// blank-line delimited, `:`-prefixed comment/keep-alive lines ignored)
// rather than the full SSE spec (no `id:`/`retry:` support, since this
// server never sends them - see `openapi.json`'s SSE operation description).

/** One parsed Server-Sent Events frame (an `event:`/`data:` pair, `data:` may span multiple lines). */
export interface SseFrame {
  event?: string;
  data: string;
}

/**
 * Thrown by {@link parseSseStream} when the underlying stream ends
 * (`reader.read()` reports `done`) while a partial frame is still sitting in
 * the buffer - i.e. the connection was cut before that frame's trailing
 * blank-line terminator arrived. This is distinct from the three normal
 * termination paths documented on `CodexAppServerRestClient.streamEvents`'s
 * doc comment (a `closed` event frame, a clean end of stream right after
 * one, or a terminal `event: error` frame): it means the transport broke
 * mid-write (a proxy reset, the server process dying mid-response, etc.).
 * Surfacing this distinctly matters because silently discarding the
 * leftover bytes - the previous behavior - makes a truncated stream
 * indistinguishable from a clean one to the caller.
 */
export class CodexAppServerRestStreamTruncatedError extends Error {
  /** The undelivered, unterminated tail of the stream, for diagnostics. */
  readonly leftover: string;

  constructor(leftover: string) {
    super(
      "codex-app-server-rest: SSE stream ended mid-frame " +
        `(${leftover.length} buffered character(s) never reached a trailing blank line) - ` +
        "the connection was likely cut before the server finished writing its last frame",
    );
    this.name = "CodexAppServerRestStreamTruncatedError";
    this.leftover = leftover;
  }
}

export function parseSseFrame(raw: string): SseFrame | null {
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
 * Splits every complete (`\n\n`-terminated) frame out of `buffer`, yielding
 * each parsed frame and returning whatever incomplete tail is left. Factored
 * out of {@link parseSseStream} so that loop isn't duplicated between the
 * "chunk arrived" path and the "flush after EOF" path below - both need to
 * drain whatever complete frames the newest bytes completed.
 */
function* drainFrames(buffer: string): Generator<SseFrame, string> {
  let boundary = buffer.indexOf("\n\n");
  while (boundary !== -1) {
    const frame = parseSseFrame(buffer.slice(0, boundary));
    if (frame) {
      yield frame;
    }
    buffer = buffer.slice(boundary + 2);
    boundary = buffer.indexOf("\n\n");
  }
  return buffer;
}

/** Parses a `text/event-stream` body into whole frames. */
export async function* parseSseStream(body: ReadableStream<Uint8Array>): AsyncGenerator<SseFrame> {
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
      buffer = yield* drainFrames(buffer);
    }
    // Flush the decoder's held-back state: `{ stream: true }` above lets it
    // hold onto the trailing bytes of a not-yet-complete multi-byte UTF-8
    // sequence, expecting a following chunk to complete it. At true EOF
    // there is no following chunk, so an unflushed decoder would silently
    // drop those bytes. Calling `decode()` with no arguments (default
    // `stream: false`) either completes that sequence or, if the stream
    // really did end mid-character, replaces it with U+FFFD rather than
    // truncating it invisibly.
    buffer += decoder.decode();
    buffer = yield* drainFrames(buffer);
    // Anything still in `buffer` here never reached a trailing blank line -
    // see `CodexAppServerRestStreamTruncatedError`'s doc comment. Whitespace
    // left over from a cleanly-terminated stream (e.g. a stray final
    // newline) doesn't count as truncation.
    if (buffer.trim().length > 0) {
      throw new CodexAppServerRestStreamTruncatedError(buffer);
    }
  } finally {
    // `releaseLock()` alone (the historical bug here) detaches the reader
    // from the stream but does NOT tell the underlying response body to
    // stop - the socket, and the server-side handler blocked on it, stay
    // open indefinitely. This matters most on early exit: a caller doing
    // `for await (const e of client.streamEvents(id)) { ...; break; }`
    // causes the JS engine to call this generator's `return()`, which
    // resumes execution here via `finally` - so `cancel()` is what actually
    // aborts the in-flight fetch and lets the server observe the close.
    // `cancel()` rejects if the stream is already closed/errored (e.g. the
    // normal "read to completion" path above, or after `decoder`'s flush
    // threw); that's not a caller-visible failure, so it's swallowed.
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}
