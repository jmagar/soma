// End-to-end regression proofs for findings 1 and 2, run against a real
// `node:http` server and the real global `fetch` (no mocking of the
// transport itself - only these two tests exercise actual sockets; see
// `test/sse.test.ts` and `test/client.test.ts` for the faster in-process
// unit tests).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";

import { CodexAppServerRestClient } from "../src/client.ts";

function listen(server: Server): Promise<number> {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        reject(new Error("failed to bind an ephemeral port"));
        return;
      }
      resolve(address.port);
    });
  });
}

function closeServer(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}

/** Races `promise` against a timeout, throwing a descriptive error instead of hanging the test suite. */
async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error(`${what} did not happen within ${timeoutMs}ms`)), timeoutMs);
  });
  try {
    return await Promise.race([promise, timeout]);
  } finally {
    // Cancels the pending timer once `promise` has already settled -
    // otherwise `timeout` would fire later and, though nothing awaits it by
    // then, an unrelated real timeout in a later test could get mixed up
    // with a still-pending timer from this one.
    clearTimeout(timer!);
  }
}

test(
  "streamEvents: breaking out of the loop closes the connection promptly and lets the server " +
    "observe it, and server.close() does not hang afterward (finding 1 regression)",
  async () => {
    let onClose: (() => void) | undefined;
    const serverSawClose = new Promise<void>((resolve) => {
      onClose = resolve;
    });

    const server = createServer((req, res) => {
      res.writeHead(200, { "content-type": "text/event-stream" });
      let n = 0;
      // Keeps writing frames indefinitely, like a real long-lived SSE
      // session with an `ActivePollGuard` held open server-side - the only
      // way this handler ever finishes is by observing the client go away.
      const interval = setInterval(() => {
        n += 1;
        res.write(`event: message\ndata: {"event":"message","n":${n}}\n\n`);
      }, 20);
      req.on("close", () => {
        clearInterval(interval);
        onClose?.();
      });
    });

    const port = await listen(server);
    try {
      const client = new CodexAppServerRestClient({ baseUrl: `http://127.0.0.1:${port}` });

      let seen = 0;
      for await (const _event of client.streamEvents("session-1")) {
        void _event;
        seen += 1;
        break;
      }
      assert.equal(seen, 1);

      // The core proof: with only `releaseLock()` (the historical bug),
      // this would never resolve on its own - the socket stays open and
      // the server keeps writing to it until an idle TTL or process death.
      // With `reader.cancel()`, the fetch aborts and the server's `req`
      // observes 'close' promptly.
      await withTimeout(serverSawClose, 2000, "the server observing the client closing the connection");
    } finally {
      // The second proof: a server with no more open connections must be
      // able to shut down cleanly - `server.close()`'s callback only fires
      // once every connection has ended.
      await withTimeout(closeServer(server), 2000, "server.close()");
    }
  },
);

test(
  "call / sessionCall: `..`-laden method or sessionId now throw before reaching the server, " +
    "instead of retargeting the request (finding 2 regression)",
  async () => {
    const seenUrls: string[] = [];
    const server = createServer((req, res) => {
      seenUrls.push(req.url ?? "");
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
    });

    const port = await listen(server);
    try {
      const client = new CodexAppServerRestClient({ baseUrl: `http://127.0.0.1:${port}` });

      for (const method of ["../sessions", "../../health", "..", "."]) {
        assert.throws(() => client.call(method, {}), TypeError, `method ${JSON.stringify(method)} should throw`);
      }
      assert.throws(() => client.sessionCall("..", "foo", {}), TypeError);
      assert.throws(() => client.sessionCall("session-1", "..", {}), TypeError);
      assert.deepEqual(seenUrls, [], "no `..`-laden request should ever have reached the server");

      // Sanity check in the other direction: a legitimate multi-segment
      // method still reaches exactly the intended route.
      await client.call("thread/start", {});
      assert.deepEqual(seenUrls, ["/v1/call/thread/start"]);
    } finally {
      await withTimeout(closeServer(server), 2000, "server.close()");
    }
  },
);
