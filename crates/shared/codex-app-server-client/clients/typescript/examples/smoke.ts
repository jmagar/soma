#!/usr/bin/env node
// Runnable example: spawns a real `codex-app-server-rest --mode trusted-bridge`
// process and drives it through `CodexAppServerRestClient`, demonstrating:
//
//   1. a bearer-auth'd call (`GET /v1/compatibility`, which requires the
//      token - unlike `/health`)
//   2. a one-shot text-turn call (`POST /v1/text-turn`)
//   3. consuming the SSE event stream (`GET /v1/sessions/{id}/events/stream`)
//
// Run with: `pnpm run smoke` (equivalent to `node examples/smoke.ts`).
//
// Requires the crate's binary already built: `cargo build -p codex-app-server-client
// --features rest --bin codex-app-server-rest` from the repo root. This script
// looks for it at `target/debug/codex-app-server-rest` (see
// `../src/internal/find-binary.ts`) and exits early with a clear message if
// it's missing, rather than failing confusingly on the first HTTP request.
//
// The text-turn step needs a working `codex` CLI *and* a configured model
// provider on this machine (see `codex login`) to actually complete a turn -
// unlike `/health`/`/v1/compatibility`, it drives a real model call. This
// script treats a text-turn failure as a reported, non-fatal outcome (prints
// the error and moves on) rather than aborting the whole demo, since "no
// model provider configured" is an environment fact this script cannot and
// should not assume - see README.md's "Running the example" section.

import { setTimeout as delay } from "node:timers/promises";
import { spawn } from "node:child_process";

import { CodexAppServerRestClient, CodexAppServerRestError } from "../src/client.ts";
import { findBinary } from "../src/internal/find-binary.ts";
import { pickFreePort } from "../src/internal/free-port.ts";

async function main(): Promise<void> {
  const binaryPath = findBinary();
  if (binaryPath === null) {
    console.log(
      "codex-app-server-rest binary not found at target/debug/codex-app-server-rest " +
        "(and CODEX_APP_SERVER_REST_BIN is not set). Build it first:\n\n" +
        "  cargo build -p codex-app-server-client --features rest --bin codex-app-server-rest\n\n" +
        "Skipping the example run.",
    );
    return;
  }

  const port = await pickFreePort();
  const token = `smoke-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  const host = "127.0.0.1";
  const baseUrl = `http://${host}:${port}`;

  console.log(`starting ${binaryPath} --mode trusted-bridge on ${baseUrl} ...`);
  const server = spawn(
    binaryPath,
    ["--host", host, "--port", String(port), "--mode", "trusted-bridge", "--token", token],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  server.stdout?.on("data", (chunk: Buffer) => process.stdout.write(`  [server] ${chunk}`));
  server.stderr?.on("data", (chunk: Buffer) => process.stderr.write(`  [server] ${chunk}`));

  try {
    const client = new CodexAppServerRestClient({ baseUrl, token });
    await waitForHealth(client);

    console.log("\n=== 1. bearer-auth'd call: GET /v1/compatibility ===");
    const compat = await client.compatibility();
    console.log(compat);

    console.log("\n=== 2. one-shot text-turn call: POST /v1/text-turn ===");
    try {
      const turn = await client.textTurn({ prompt: "Reply with exactly the word: pong" });
      console.log(turn);
    } catch (error) {
      if (error instanceof CodexAppServerRestError) {
        console.log(
          `text-turn did not complete (status=${error.status}, error=${error.body.error}: ` +
            `${error.body.message}). This usually means no model provider is configured on ` +
            "this machine (see `codex login`) - not a client bug. Continuing with the demo.",
        );
      } else {
        throw error;
      }
    }

    console.log("\n=== 3. consuming the SSE event stream ===");
    const session = await client.createSession();
    console.log(`created session ${session.sessionId}`);
    try {
      let seen = 0;
      for await (const event of client.streamEvents(session.sessionId, 1000)) {
        console.log(`  sse frame: ${event.event}`);
        seen += 1;
        if (seen >= 3 || event.event === "closed") {
          break;
        }
      }
      console.log(`consumed ${seen} SSE frame(s) (each request's server-side stream ends when this loop stops reading it)`);
    } finally {
      await client.deleteSession(session.sessionId);
      console.log(`deleted session ${session.sessionId}`);
    }
  } finally {
    server.kill("SIGTERM");
    await delay(200);
    if (!server.killed) {
      server.kill("SIGKILL");
    }
  }
}

async function waitForHealth(client: CodexAppServerRestClient, attempts = 50): Promise<void> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      await client.health();
      return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("server did not become healthy in time");
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
