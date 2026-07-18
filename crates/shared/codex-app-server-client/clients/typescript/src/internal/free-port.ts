// Internal helper shared by `examples/smoke.ts` and (in inline, duplicated
// form - see that file's own comment) `scripts/live-smoke.mjs`: picks an
// ephemeral free TCP port on loopback by asking the OS for port `0` and
// reading back what it bound, then immediately releasing it so
// `codex-app-server-rest` can bind the same port a moment later. Small
// TOCTOU race in theory (another process could grab the port in between);
// acceptable here since this only drives local example/smoke scripts, never
// production code.
import { createServer } from "node:net";

export function pickFreePort(): Promise<number> {
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
      server.close((closeError) => {
        if (closeError) {
          reject(closeError);
        } else {
          resolve(port);
        }
      });
    });
  });
}
