// Internal helper (not re-exported from `../index.ts`) shared by
// `examples/smoke.ts` and `scripts/live-smoke.mjs`'s equivalent inline
// logic: locates the workspace-built `codex-app-server-rest` debug binary
// relative to this file, without hardcoding an absolute path.

import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * `crates/shared/codex-app-server-client/clients/typescript/src/internal/` is
 * five directories below the repo root, which is where `cargo build`'s
 * default `target/debug/` lives.
 */
export function defaultBinaryPath(): string {
  const __dirname = path.dirname(fileURLToPath(import.meta.url));
  // src/internal/ -> src/ -> typescript/ -> clients/ -> codex-app-server-client/
  //   -> shared/ -> crates/ -> repo root (7 levels).
  const repoRoot = path.resolve(__dirname, "../../../../../../..");
  const binaryName = process.platform === "win32" ? "codex-app-server-rest.exe" : "codex-app-server-rest";
  return path.join(repoRoot, "target", "debug", binaryName);
}

export function findBinary(): string | null {
  const fromEnv = process.env.CODEX_APP_SERVER_REST_BIN;
  if (fromEnv && existsSync(fromEnv)) {
    return fromEnv;
  }
  const defaultPath = defaultBinaryPath();
  return existsSync(defaultPath) ? defaultPath : null;
}
