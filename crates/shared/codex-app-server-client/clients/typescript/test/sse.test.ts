// Unit tests for the hand-rolled SSE parser in `../src/internal/sse.ts`,
// exercised directly against synthetic `ReadableStream`s rather than through
// a real `fetch()`. That gives byte-exact control over chunk boundaries
// (confirmed empirically: a `ReadableStream` built by `enqueue`-ing N chunks
// in `start()` delivers exactly N `read()` results, in order - see the repo
// history for the throwaway script that verified this before these tests
// were written), which is what's needed to pin down the chunk-boundary and
// decoder-flush bugs this suite covers.
import { test } from "node:test";
import assert from "node:assert/strict";

import { CodexAppServerRestStreamTruncatedError, parseSseFrame, parseSseStream } from "../src/internal/sse.ts";

const encoder = new TextEncoder();

/** Builds a `ReadableStream` that delivers exactly `chunks.length` `read()` results, in order. */
function streamFromChunks(chunks: Uint8Array[]): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(chunk);
      }
      controller.close();
    },
  });
}

async function collect(stream: ReadableStream<Uint8Array>) {
  const frames = [];
  for await (const frame of parseSseStream(stream)) {
    frames.push(frame);
  }
  return frames;
}

test("parseSseFrame: ignores blank and `:`-prefixed comment/keep-alive lines", () => {
  const frame = parseSseFrame(": keep-alive\nevent: tick\ndata: 1\n: another comment\ndata: 2");
  assert.deepEqual(frame, { event: "tick", data: "1\n2" });
});

test("parseSseFrame: a comment-only frame (bare keep-alive, no data:) parses to null", () => {
  assert.equal(parseSseFrame(": keep-alive\n: still just a comment"), null);
});

test("parseSseFrame: multiple data: lines are joined with \\n, per the SSE spec", () => {
  const frame = parseSseFrame("data: line one\ndata: line two\ndata: line three");
  assert.deepEqual(frame, { data: "line one\nline two\nline three" });
});

test("parseSseFrame: a frame with no event: field omits `event` from the result", () => {
  const frame = parseSseFrame('data: {"x":1}');
  assert.deepEqual(frame, { data: '{"x":1}' });
});

test("parseSseStream: splits frames delivered across an arbitrary chunk boundary", async () => {
  const full = encoder.encode('event: a\ndata: {"n":1}\n\nevent: b\ndata: {"n":2}\n\n');
  const splitAt = 10; // lands inside the first frame's data line, not aligned to any delimiter
  const frames = await collect(streamFromChunks([full.slice(0, splitAt), full.slice(splitAt)]));
  assert.deepEqual(frames, [
    { event: "a", data: '{"n":1}' },
    { event: "b", data: '{"n":2}' },
  ]);
});

test("parseSseStream: splits a chunk boundary landing exactly inside the `\\n\\n` frame delimiter", async () => {
  const text = "event: a\ndata: 1\n\nevent: b\ndata: 2\n\n";
  const boundary = text.indexOf("\n\n") + 1; // between the delimiter's two newlines
  const frames = await collect(
    streamFromChunks([encoder.encode(text.slice(0, boundary)), encoder.encode(text.slice(boundary))]),
  );
  assert.deepEqual(frames, [
    { event: "a", data: "1" },
    { event: "b", data: "2" },
  ]);
});

test("parseSseStream: reassembles a multi-byte UTF-8 character split across a chunk boundary", async () => {
  const full = encoder.encode("event: emoji\ndata: \u{1F389}done\n\n"); // U+1F389 is a 4-byte UTF-8 sequence
  const leadByteIndex = full.indexOf(0xf0);
  assert.ok(leadByteIndex !== -1, "test setup: expected to find the emoji's lead byte");
  const splitAt = leadByteIndex + 2; // split 2 bytes into the 4-byte sequence - a following chunk completes it
  const frames = await collect(streamFromChunks([full.slice(0, splitAt), full.slice(splitAt)]));
  assert.deepEqual(frames, [{ event: "emoji", data: "\u{1F389}done" }]);
});

test("parseSseStream: bare comment-only frames between real frames are silently skipped", async () => {
  const text = ": keep-alive\n\nevent: a\ndata: 1\n\n: keep-alive\n\nevent: b\ndata: 2\n\n";
  const frames = await collect(streamFromChunks([encoder.encode(text)]));
  assert.deepEqual(frames, [
    { event: "a", data: "1" },
    { event: "b", data: "2" },
  ]);
});

test("parseSseStream: throws CodexAppServerRestStreamTruncatedError when EOF arrives mid-frame (missing trailing blank line)", async () => {
  // The second frame is never terminated by a blank line - simulates a
  // connection cut mid-write (proxy reset, server panic, etc.).
  const text = "event: a\ndata: 1\n\nevent: b\ndata: 2";
  const stream = streamFromChunks([encoder.encode(text)]);
  const frames: unknown[] = [];
  let caught: unknown;
  try {
    for await (const frame of parseSseStream(stream)) {
      frames.push(frame);
    }
  } catch (error) {
    caught = error;
  }
  // The well-terminated first frame is still delivered before the failure.
  assert.deepEqual(frames, [{ event: "a", data: "1" }]);
  assert.ok(caught instanceof CodexAppServerRestStreamTruncatedError);
  assert.ok(caught.leftover.includes("event: b"));
});

test("parseSseStream: flushes the decoder at EOF so a dangling multi-byte lead byte is surfaced, not silently dropped", async () => {
  // A lone 0xf0 is the first byte of a 4-byte UTF-8 sequence with no
  // continuation bytes ever following - the connection died mid-character.
  // Without flushing the decoder after the read loop, `TextDecoder.decode(x,
  // { stream: true })` holds this byte back forever and it never enters
  // `buffer` at all - which would make `buffer` end up empty (all earlier
  // frames were already complete and drained) and the truncation go
  // completely undetected. This is the sharpest reproduction of both halves
  // of finding 3 at once.
  const frame1 = encoder.encode("event: hello\ndata: 1\n\n");
  const danglingLeadByte = new Uint8Array([0xf0]);
  const stream = streamFromChunks([frame1, danglingLeadByte]);
  const frames: unknown[] = [];
  let caught: unknown;
  try {
    for await (const frame of parseSseStream(stream)) {
      frames.push(frame);
    }
  } catch (error) {
    caught = error;
  }
  assert.deepEqual(frames, [{ event: "hello", data: "1" }]);
  assert.ok(caught instanceof CodexAppServerRestStreamTruncatedError);
  // If the decoder were never flushed, `leftover` would be empty and this
  // truncation would have gone completely unnoticed.
  assert.ok(caught.leftover.length > 0);
});

test("parseSseStream: a cleanly terminated stream (final frame ends with a blank line) does not throw", async () => {
  const text = "event: a\ndata: 1\n\nevent: closed\ndata: 2\n\n";
  const frames = await collect(streamFromChunks([encoder.encode(text)]));
  assert.deepEqual(frames, [
    { event: "a", data: "1" },
    { event: "closed", data: "2" },
  ]);
});

test("parseSseStream: breaking out of a `for await` loop early cancels the underlying reader (regression for the connection-leak bug)", async () => {
  let cancelCalled = false;
  let cancelReason: unknown;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(encoder.encode("event: a\ndata: 1\n\n"));
      // Deliberately never closes or errors - simulates a live SSE
      // connection with more frames still to come. The only way this
      // generator "finishes" here is via the consumer breaking out of its
      // `for await` loop below, which is exactly the scenario that used to
      // leak: `releaseLock()` alone detaches the reader without telling
      // this `cancel()` callback (and, in a real fetch, the underlying
      // socket) anything.
    },
    cancel(reason) {
      cancelCalled = true;
      cancelReason = reason;
    },
  });
  for await (const frame of parseSseStream(stream)) {
    assert.deepEqual(frame, { event: "a", data: "1" });
    break;
  }
  assert.equal(cancelCalled, true);
  assert.equal(cancelReason, undefined); // `reader.cancel()` is called with no reason
});
