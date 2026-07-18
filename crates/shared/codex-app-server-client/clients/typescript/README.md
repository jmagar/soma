# codex-app-server-rest-client (TypeScript)

A checked-in, generated TypeScript client for the `codex-app-server-client`
crate's `rest` feature - proof that `../../openapi.json` (the crate's
checked-in OpenAPI 3.1.0 spec, built by `src/rest/openapi.rs`) is consumable
outside Rust, not just readable by it.

This package is **standalone**: it is not part of any pnpm workspace, has no
`package.json` above it, and is not referenced by `apps/web` or
`packages/soma-rmcp`. It lives under the crate specifically because the
crate itself has zero workspace path-dependencies (see the crate's own
README.md) and can be lifted wholesale into another repo - a client
generated from *its* spec has to travel with it.

## What's here

```
clients/typescript/
  package.json            private, unpublished - devDependencies only
  pnpm-lock.yaml           checked in (pnpm install never runs un-pinned in CI)
  tsconfig.json
  .gitignore               node_modules/, .cache/, *.tsbuildinfo
  src/
    generated/
      openapi-types.ts     GENERATED - do not hand-edit (see "Regenerating" below)
    internal/
      find-binary.ts        locates target/debug/codex-app-server-rest
      free-port.ts           picks an ephemeral loopback port
    client.ts               hand-written: CodexAppServerRestClient + SSE parsing
    index.ts                barrel export
  scripts/
    generate.mjs             regenerates src/generated/openapi-types.ts (--check to verify only)
    live-smoke.mjs           starts the real binary, hits /health + /v1/compatibility
  examples/
    smoke.ts                 runnable example: text-turn, bearer auth, SSE stream
```

## Why generated types + a thin hand-written wrapper (not a full generated client)

Two real options exist for "generate a TypeScript client from an OpenAPI
spec": generate *types only* (`openapi-typescript`) and hand-write a thin
runtime on top, or generate a *full client* (`@hey-api/openapi-ts`,
`openapi-generator-cli`, ...) that also emits the fetch/request logic.

This package takes the first path:

- **The whole point of a *checked-in* generated artifact is that a reviewer
  can read it in a diff.** `openapi-typescript`'s output is a flat set of
  `interface`/type declarations - a PR that changes a route's response shape
  produces a small, obviously-correct diff in `openapi-types.ts`. Full
  client generators (especially `openapi-generator-cli`, which shells out to
  a Java JAR) tend to emit large amounts of generated *runtime* code
  (request builders, serializers, a generated `ApiClient` class per tag) that
  reviewers scroll past rather than read.
- **Zero runtime dependencies.** `openapi-typescript`'s output has none - it's
  pure TypeScript types, erased at compile time. `client.ts` (hand-written,
  ~330 lines) only uses platform `fetch`/`URL`/`TextDecoder`/`ReadableStream`,
  available in every evergreen browser and Node.js >=18. A full generated
  client typically pulls in its own HTTP wrapper package as a real
  `dependencies` entry.
- **This API's shape doesn't need a heavy client.** 13 routes, one bearer
  token, one non-standard path parameter (see below), and one SSE endpoint.
  Hand-writing that thin layer is less code, and more auditable code, than
  configuring a full generator's templates to produce the same thing.

`openapi-typescript` was chosen over `@hey-api/openapi-ts` on the same
minimalism argument: `openapi-typescript` is the smaller, single-purpose tool
(types only), and this package's hand-written `client.ts` covers the "full
client" ground `@hey-api/openapi-ts` would otherwise generate.

## Install

```sh
cd crates/shared/codex-app-server-client/clients/typescript
pnpm install
```

Requires Node.js >=20.19 and pnpm (both available via `mise` in this repo's
toolchain - see the root `CLAUDE.md`). `node_modules/` is gitignored; nothing
in this package is published to a registry (`"private": true`).

## Regenerating `src/generated/openapi-types.ts`

```sh
pnpm run generate       # overwrite src/generated/openapi-types.ts
pnpm run check-sync      # verify it's already up to date; exits 1 if not
```

Both commands run `scripts/generate.mjs`, which calls `openapi-typescript`'s
JS API (not its CLI - see that script's own doc comment) against
`../../openapi.json`. The build is deterministic: byte-identical output on
every run, given the same `openapi.json` (verified as part of this package's
own change history - regenerate twice, `git diff --exit-code` the result).

### The spec is consumed exactly as checked in

`generate.mjs` hands `openapi.json` to `openapi-typescript` verbatim - no
transforms, no patching, no in-memory fixups. Keep it that way: if a
generator can't read the spec as-is, then neither can anyone else's, and the
fix belongs in `../../src/rest/openapi.rs` rather than in a workaround here.
A spec that only works after this package massages it is not the portable
artifact it claims to be.

That rule has already paid for itself once. Building this client is what
surfaced a real bug in the spec: `RestEventResponse` carried a
`discriminator.mapping` pointing at four
`#/components/schemas/RestEventResponse{Notification,Request,Closed,Timeout}`
entries that were only ever built inline inside the `oneOf` array and never
registered as components. `openapi-typescript`'s Redocly-based validator
rejected the whole document ("Can't resolve $ref at
.../discriminator/mapping/*"), and so would any other spec-compliant
generator. It is fixed at the source now - the four variants are real named
schemas that the `oneOf` `$ref`s and the mapping both point at - and
`openapi.rs`'s `every_schema_ref_resolves_to_a_real_component` test fails the
Rust build if a ref ever dangles again.

## The sync check: `cargo xtask check-ts-client`

Wired as `xtask/src/ts_client.rs`, following the same `--write`/`--check`
convention as `cargo xtask check-openapi` (see `xtask/src/scripts_lane_d.rs`):

```sh
cargo xtask check-ts-client --write   # regenerate + overwrite
cargo xtask check-ts-client --check   # verify sync, then `pnpm run typecheck`
cargo xtask check-ts-client           # same as --check (the default)
```

It shells out to this package's own `pnpm` scripts rather than reimplementing
`openapi-typescript` in Rust - the whole point of this package is proving a
real, independent TypeScript toolchain can consume `openapi.json`;
reimplementing that logic in the Rust check would defeat the proof. If
`node` or `pnpm` is missing from `PATH`, the check prints a message and
exits `0` (skipped, not failed) - this repo's self-hosted CI runners are not
guaranteed to have a Node toolchain provisioned, and a hard failure there
would read as flakiness rather than a real problem with a given change. This
mirrors `cargo xtask codex-schema drift`'s posture on a missing `codex` CLI
(see `xtask/src/codex_schema/drift.rs`).

**Why xtask, not a bare `package.json` script wired directly into CI:** this
package has no build/install step at CI-trigger time by itself - something
has to decide *whether* to run it (node/pnpm present?) and how to report a
skip distinctly from a pass or fail, and `xtask` is already the single
front door this repo's CI uses for every other "is a generated artifact
still in sync" check (`check-openapi`, `check-schema-docs`, ...). Putting
the skip logic in `xtask` keeps that "one place to look" property; a bare
`pnpm run check-sync` in a workflow step would either hard-fail when
node/pnpm is absent or need its own bespoke presence-checking shell, applied
nowhere else in this repo's CI.

CI wiring: `.github/workflows/ci.yml`'s `soma` job (path-gated on
`needs.changes.outputs.soma`, which `xtask/src/ci_paths.rs` sets whenever
anything under `crates/` changes) runs `cargo xtask check-ts-client --check`
alongside its other generated-artifact checks.

## The `{method}` wildcard

`POST /v1/call/{method}` and `POST /v1/sessions/{sessionId}/call/{method}`
capture `{method}` via an axum `{*method}` catch-all (see `../../src/rest/routes.rs`),
because a real `codex app-server` JSON-RPC method name is namespaced with a
literal `/` (`thread/start`, `config/read`, ...). `openapi.json`'s own
parameter description warns that "naive path-templating clients that escape
`/` in path parameters will build the wrong URL."

**Verified against a live `codex-app-server-rest --mode trusted-bridge`
instance** (both forms sent with `-d '{}'` and a valid bearer token):

```text
$ curl -X POST http://127.0.0.1:PORT/v1/call/thread/start ...
HTTP 502 {"error":"json_rpc_error","message":"Invalid request: missing field `params`","code":-32600}

$ curl -X POST http://127.0.0.1:PORT/v1/call/thread%2Fstart ...
HTTP 502 {"error":"json_rpc_error","message":"Invalid request: missing field `params`","code":-32600}
```

Both requests reached the *same* handler and produced byte-identical error
bodies (a real JSON-RPC error from the spawned `codex app-server` process
complaining about the request `thread/start` itself, not a 404/400 from
routing) - axum/hyper decode `%2F` back to `/` before wildcard matching, so a
naive `encodeURIComponent(method)` happens to still work against this
specific server.

This package's `encodeMethodPath()` (in `src/client.ts`) does **not** rely on
that decode-before-match behavior. It splits `method` on `/`, percent-encodes
only the characters *within* each segment, and rejoins with literal `/` -
matching what `openapi.json`'s own parameter description documents as the
honest shape of the value, and remaining correct even against an
intermediary that doesn't share axum's specific decoding behavior.

## Running the example

```sh
pnpm run smoke
```

Builds on `cargo build -p codex-app-server-client --features rest --bin
codex-app-server-rest` having already produced `target/debug/codex-app-server-rest`
(the example looks for it there, or at `$CODEX_APP_SERVER_REST_BIN`, and
prints a clear skip message and returns cleanly if it's missing - it does not
build the binary itself). It spawns that binary in `--mode trusted-bridge`
on an ephemeral loopback port with a random token, then:

1. calls `GET /v1/compatibility` (bearer-auth'd - unlike `/health`, this
   route requires the token);
2. calls `POST /v1/text-turn` with a short prompt. This step drives a real
   model call through whatever `codex` provider is configured on the host
   (see `codex login`) - if none is configured, the example reports the
   failure and continues rather than aborting the whole demo, since that's
   an environment fact this script can't assume either way;
3. opens a bridge session and consumes its SSE event stream
   (`GET /v1/sessions/{id}/events/stream`) for a few frames.

## Running TypeScript directly with `node`

Every script and example in this package (`scripts/generate.mjs` aside,
which is plain `.mjs`) runs directly via `node examples/smoke.ts` /
`node scripts/live-smoke.mjs`'s `import "../src/client.ts"` with **no**
build step, `ts-node`, or `tsx` - Node.js's built-in TypeScript support
(stable by default since Node 23.6) strips type annotations at load time.
Two consequences that shaped how this package's source is written:

- Imports use the real `.ts` extension (`import { X } from "./client.ts"`),
  not the `tsc`-NodeNext convention of writing `.js` and letting the
  compiler resolve it to the sibling `.ts` file - Node's loader resolves
  exactly what's on disk. `tsconfig.json` sets `allowImportingTsExtensions`
  (paired with `noEmit`, since this package never compiles to `.js`) so
  `tsc --noEmit` accepts the same imports.
- No TypeScript constructor parameter properties (`constructor(public
  readonly x: T)`) - that syntax lowers to real runtime field-assignment
  code, which Node's "erasable syntax only" stripping mode rejects with
  `ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX`. `CodexAppServerRestError` in
  `src/client.ts` uses explicit field declarations + manual assignment
  instead.

## Live smoke: proving this actually talks to the real server

```sh
pnpm run live-smoke
```

Starts `codex-app-server-rest --mode health-only` (no `codex` subprocess, no
model call - see that mode's description in `../../src/bin/codex_app_server_rest.rs`),
then calls `GET /health` and `GET /v1/compatibility` for real through
`CodexAppServerRestClient` and checks the response shapes. Skips gracefully
(prints why, exits 0) if `target/debug/codex-app-server-rest` isn't built -
see `scripts/live-smoke.mjs`'s own comment.
