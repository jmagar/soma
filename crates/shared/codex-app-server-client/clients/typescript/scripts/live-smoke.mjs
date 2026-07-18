#!/usr/bin/env node
// Starts a real `codex-app-server-rest --mode health-only` process and hits
// `GET /health` and `GET /v1/compatibility` through `CodexAppServerRestClient`
// - the specific proof this package's bead (see clients/typescript/README.md)
// requires: that the generated types + hand-written client actually work
// against the real server, not just that they type-check.
//
// `--mode health-only` mounts no executing route beyond health/compatibility
// (see codex_app_server_rest.rs's MODES section), so this script never spawns
// a `codex` subprocess or drives a model call - unlike `examples/smoke.ts`,
// it has no dependency on a configured model provider.
//
// Skips gracefully (prints why, exits 0) when the binary isn't built, since
// building the Rust crate is out of scope for a TypeScript package script -
// see xtask/src/ts_client.rs's own "node/pnpm missing" skip for the same
// posture from the other direction.
//
// Usage: `pnpm run live-smoke` (equivalent to `node scripts/live-smoke.mjs`).

import { setTimeout as delay } from "node:timers/promises";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { CodexAppServerRestClient } from "../src/client.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// scripts/ -> typescript/ -> clients/ -> codex-app-server-client/ -> shared/ -> crates/ -> repo root (6 levels).
const repoRoot = path.resolve(__dirname, "../../../../../..");
const binaryName = process.platform === "win32" ? "codex-app-server-rest.exe" : "codex-app-server-rest";
const defaultBinaryPath = path.join(repoRoot, "target", "debug", binaryName);

function findBinary() {
  const fromEnv = process.env.CODEX_APP_SERVER_REST_BIN;
  if (fromEnv && existsSync(fromEnv)) {
    return fromEnv;
  }
  return existsSync(defaultBinaryPath) ? defaultBinaryPath : null;
}

function pickFreePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        server.close();
        reject(new Error("failed to determine an ephemeral port"));
        return;
      }
      const { port } = address;
      server.close((closeError) => (closeError ? reject(closeError) : resolve(port)));
    });
  });
}

async function waitForHealth(client, attempts = 50) {
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

/**
 * Sends `SIGTERM`, then waits for the child to actually exit before
 * returning - not just for the signal to be delivered. `ChildProcess.killed`
 * (what the code here used to check) flips to `true` synchronously the
 * moment `kill()` is called; it does not mean the process has exited, so a
 * `if (!server.killed) server.kill("SIGKILL")` fallback right after it is
 * dead code that can never run. Escalates to `SIGKILL` if the process is
 * still running after `gracefulTimeoutMs`, so a slow-to-exit
 * `codex-app-server-rest` doesn't get orphaned when this script exits.
 */
async function stopServer(server, gracefulTimeoutMs = 2000) {
  if (server.exitCode !== null || server.signalCode !== null) {
    return;
  }
  server.kill("SIGTERM");
  if (await waitForExit(server, gracefulTimeoutMs)) {
    return;
  }
  server.kill("SIGKILL");
  if (!(await waitForExit(server, 5000))) {
    console.error(`  [live-smoke] warning: pid ${server.pid} did not exit even after SIGKILL`);
  }
}

function waitForExit(server, timeoutMs) {
  return new Promise((resolve) => {
    const onExit = () => {
      clearTimeout(timer);
      resolve(true);
    };
    const timer = setTimeout(() => {
      server.off("exit", onExit);
      resolve(false);
    }, timeoutMs);
    server.once("exit", onExit);
  });
}

async function main() {
  const binaryPath = findBinary();
  if (binaryPath === null) {
    console.log(
      `codex-app-server-rest binary not found at ${defaultBinaryPath} ` +
        "(and CODEX_APP_SERVER_REST_BIN is not set). Build it first:\n\n" +
        "  cargo build -p codex-app-server-client --features rest --bin codex-app-server-rest\n\n" +
        "Skipping the live smoke check (exit 0).",
    );
    return;
  }

  const port = await pickFreePort();
  const host = "127.0.0.1";
  const baseUrl = `http://${host}:${port}`;

  console.log(`starting ${binaryPath} --mode health-only on ${baseUrl} ...`);
  const server = spawn(binaryPath, ["--host", host, "--port", String(port), "--mode", "health-only"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  server.stdout.on("data", (chunk) => process.stdout.write(`  [server] ${chunk}`));
  server.stderr.on("data", (chunk) => process.stderr.write(`  [server] ${chunk}`));

  try {
    const client = new CodexAppServerRestClient({ baseUrl });
    await waitForHealth(client);

    console.log("\n=== GET /health ===");
    const health = await client.health();
    console.log(health);
    if (health.status !== "ok") {
      throw new Error(`unexpected /health response: ${JSON.stringify(health)}`);
    }

    console.log("\n=== GET /v1/compatibility ===");
    const compat = await client.compatibility();
    console.log(compat);
    if (typeof compat.schema_codex_version !== "string" || typeof compat.surface !== "object") {
      throw new Error(`unexpected /v1/compatibility response: ${JSON.stringify(compat)}`);
    }

    console.log("\nlive smoke OK: both routes answered with real, schema-shaped responses.");
  } finally {
    await stopServer(server);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
