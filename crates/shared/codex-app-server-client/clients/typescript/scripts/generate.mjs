#!/usr/bin/env node
// Regenerates `src/generated/openapi-types.ts` from the crate's checked-in
// `openapi.json` (the source of truth - see ../../openapi.json, built by
// ../../src/rest/openapi.rs).
//
// Usage:
//   node scripts/generate.mjs           # write src/generated/openapi-types.ts
//   node scripts/generate.mjs --check   # exit 1 if the checked-in file is stale
//
// `pnpm run generate` / `pnpm run check-sync` are thin aliases for the two
// modes above. `cargo xtask check-ts-client [--write|--check]` (see
// xtask/src/ts_client.rs) shells out to whichever of those two matches its
// own `--write`/`--check` flag, after confirming `node`/`pnpm` are on PATH.
//
// The spec is consumed exactly as checked in - no transforms, no patching.
// That is deliberate and worth keeping that way: if `openapi-typescript`
// cannot read `openapi.json` as-is, then neither can anyone else's generator,
// and the fix belongs in `openapi.rs` rather than in a workaround here. (That
// rule has already earned its keep once - the spec used to carry a
// `discriminator.mapping` pointing at component schemas that were never
// registered, which every spec-compliant generator rejects. It is fixed at the
// source now, and guarded there by openapi.rs's
// `every_schema_ref_resolves_to_a_real_component` test.)

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString, COMMENT_HEADER } from "openapi-typescript";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_PATH = path.resolve(__dirname, "../src/generated/openapi-types.ts");
// clients/typescript/scripts/ -> ../../../openapi.json
const SOURCE_SPEC_PATH = path.resolve(__dirname, "../../../openapi.json");
const CHECK_MODE = process.argv.includes("--check");

async function render() {
  const spec = JSON.parse(readFileSync(SOURCE_SPEC_PATH, "utf8"));
  const ast = await openapiTS(spec, { silent: true });
  // Matches openapi-typescript's own CLI output exactly (see its
  // bin/cli.js::generateSchema) - same header, same astToString() call, same
  // default options - so the checked-in file reads exactly like what
  // `openapi-typescript openapi.json -o ...` would produce.
  return `${COMMENT_HEADER}${astToString(ast)}`;
}

async function main() {
  const rendered = await render();
  const relOutput = path.relative(process.cwd(), OUTPUT_PATH);

  if (CHECK_MODE) {
    const current = existsSync(OUTPUT_PATH) ? readFileSync(OUTPUT_PATH, "utf8") : null;
    if (current === rendered) {
      console.log(`ok: ${relOutput} matches openapi.json`);
      return;
    }
    console.error(`FAIL: ${relOutput} is out of date with ../../openapi.json.`);
    console.error("Regenerate with: pnpm run generate");
    process.exitCode = 1;
    return;
  }

  mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
  writeFileSync(OUTPUT_PATH, rendered);
  console.log(`wrote ${relOutput}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
