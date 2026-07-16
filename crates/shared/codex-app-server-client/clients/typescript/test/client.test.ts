// Unit tests for `../src/client.ts` that don't need a real HTTP server: path
// segment validation (via `encodeMethodPath` directly, and via the public
// methods with an injected `fetch` spy - see `CodexAppServerRestClientOptions.fetch`'s
// own doc comment, which exists exactly for this purpose) and the terminal
// `event: error` SSE frame (via a real `Response`/`ReadableStream` built
// in-process, no sockets involved). See `test/mock-server.test.ts` for the
// live-socket regression proofs (findings 1 and 2 end-to-end).
import { test } from "node:test";
import assert from "node:assert/strict";

import { CodexAppServerRestClient, CodexAppServerRestError, encodeMethodPath } from "../src/client.ts";

test("encodeMethodPath: accepts a multi-segment method and leaves `/` unescaped", () => {
  assert.equal(encodeMethodPath("thread/start"), "thread/start");
});

test("encodeMethodPath: accepts a single-segment method", () => {
  assert.equal(encodeMethodPath("ping"), "ping");
});

test("encodeMethodPath: percent-encodes special characters within a segment", () => {
  assert.equal(encodeMethodPath("weird segment"), "weird%20segment");
});

test("encodeMethodPath: rejects an empty method", () => {
  assert.throws(() => encodeMethodPath(""), TypeError);
});

for (const method of ["..", ".", "../sessions", "../../health", "thread/..", "a//b"]) {
  test(`encodeMethodPath: rejects ${JSON.stringify(method)} (dot-segment / empty-segment path traversal)`, () => {
    assert.throws(() => encodeMethodPath(method), TypeError);
  });
}

function neverCalledFetch(called: { value: boolean }): typeof fetch {
  return () => {
    called.value = true;
    throw new Error("fetch should not have been called - validation should have thrown first");
  };
}

test("deleteSession: rejects a `..` sessionId before making any request", () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  assert.throws(() => client.deleteSession(".."), TypeError);
  assert.equal(called.value, false);
});

test("sessionCall: rejects a `..` sessionId (escaping the session scope) before making any request", () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  assert.throws(() => client.sessionCall("..", "foo", {}), TypeError);
  assert.equal(called.value, false);
});

test("sessionCall: rejects a `..` method before making any request", () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  assert.throws(() => client.sessionCall("session-1", "..", {}), TypeError);
  assert.equal(called.value, false);
});

test("pollEvents: rejects an empty sessionId before making any request", () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  assert.throws(() => client.pollEvents(""), TypeError);
  assert.equal(called.value, false);
});

test("replyResult / replyError: reject a `..` requestKey before making any request", () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  assert.throws(() => client.replyResult("session-1", "..", { result: null }), TypeError);
  assert.throws(() => client.replyError("session-1", "..", { code: -32000, message: "x" }), TypeError);
  assert.equal(called.value, false);
});

test("streamEvents: rejects a `..` sessionId before making any request (async generator body runs lazily on first next())", async () => {
  const called = { value: false };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: neverCalledFetch(called) });
  const iterator = client.streamEvents("..");
  await assert.rejects(() => iterator.next(), TypeError);
  assert.equal(called.value, false);
});

test("call: a well-formed request reaches fetch with the expected URL", async () => {
  const requestedUrls: string[] = [];
  const fetchSpy: typeof fetch = async (input) => {
    requestedUrls.push(input.toString());
    return new Response(JSON.stringify({ ok: true }), { status: 200 });
  };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: fetchSpy });
  await client.call("thread/start", {});
  assert.deepEqual(requestedUrls, ["http://example.invalid/v1/call/thread/start"]);
});

test("streamEvents: a terminal `event: error` frame throws CodexAppServerRestError with status null", async () => {
  const errorBody = { error: "boom", message: "something broke" };
  const sseText = `event: error\ndata: ${JSON.stringify(errorBody)}\n\n`;
  const fetchSpy: typeof fetch = async () => {
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(sseText));
        controller.close();
      },
    });
    return new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } });
  };
  const client = new CodexAppServerRestClient({ baseUrl: "http://example.invalid", fetch: fetchSpy });

  let caught: unknown;
  try {
    for await (const _event of client.streamEvents("session-1")) {
      void _event; // unreachable - the error frame throws before any event yields
    }
  } catch (error) {
    caught = error;
  }
  assert.ok(caught instanceof CodexAppServerRestError);
  assert.equal(caught.status, null);
  assert.equal(caught.body.error, "boom");
  assert.equal(caught.body.message, "something broke");
});
