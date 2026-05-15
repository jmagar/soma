# Web UI

The optional web UI lives under `apps/web/` and is built as a static Next.js export embedded into the Rust binary at compile time using `include_dir!`. No separate file-serving process.

## Build flow

```
apps/web/           ← Next.js app source
  next.config.ts    ← output: "export" (static HTML/CSS/JS)
  out/              ← compiled static output (gitignored, built in CI)

src/web.rs          ← Rust: embeds out/ into binary with include_dir!
```

## Commands

```bash
just build-web       # build apps/web/out
just web-watch       # rebuild on changes
just build-full      # build web then release binary (CI)
pnpm -C apps/web check
pnpm -C apps/web typecheck
pnpm -C apps/web build
```

## Embedding in Rust

```rust
use include_dir::{Dir, include_dir};

// Compiled at build time — zero runtime file I/O
static WEB_ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/apps/web/out");

pub fn web_assets_available() -> bool {
    WEB_ASSETS.get_file("index.html").is_some()
}

pub async fn serve_web_assets(request: Request<Body>) -> Response {
    let path = request.uri().path().trim_start_matches('/');

    // Try exact path, then with .html, then index.html (SPA fallback)
    let candidates = [
        path.to_string(),
        format!("{path}.html"),
        format!("{path}/index.html"),
        "index.html".to_string(),
    ];

    for candidate in &candidates {
        if let Some(file) = WEB_ASSETS.get_file(candidate) {
            let content_type = guess_mime(candidate);
            let cache_control = if candidate == "index.html" {
                "no-store"  // SPA shell must not be cached
            } else {
                "public, max-age=31536000, immutable"  // hashed assets = forever
            };
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type),
                 (header::CACHE_CONTROL, cache_control)],
                file.contents().to_vec(),
            ).into_response();
        }
    }

    // 404 → SPA fallback (client-side routing handles the rest)
    // ...
}
```

## Build script (build.rs)

```rust
fn main() {
    println!("cargo:rerun-if-changed=apps/web/src");
    println!("cargo:rerun-if-changed=apps/web/package.json");

    let out_dir = std::path::Path::new("apps/web/out");
    if !out_dir.exists() {
        let status = std::process::Command::new("pnpm")
            .args(["--dir", "apps/web", "build"])
            .status();
        if let Err(e) = status {
            // Don't fail the Rust build — web UI will be unavailable
            println!("cargo:warning=Web build failed: {e}.");
        }
    }
}
```

## Feature gate

The web feature is optional:

```toml
# Cargo.toml
[features]
default = ["web"]
web = ["dep:include_dir"]

[dependencies]
include_dir = { version = "0.7", optional = true }
```

## Runtime configuration

`apps/web/lib/template.ts` defines the service display name, endpoints, and optional API base URL. `NEXT_PUBLIC_EXAMPLE_API_BASE_URL` should be empty by default so the UI uses same-origin API calls when served by the Rust binary.

Use `apps/web/.env.example` for local web development overrides only.

## Static export configuration

```typescript
// apps/web/next.config.ts
const config = {
  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
  basePath: "",
};
```

## API surfaces

The UI calls:
- `/health`
- `/status`
- `/v1/example`
- `/mcp` for MCP clients rather than browser UI calls

## Aurora design system

The web UI uses the Aurora design system — shadcn-compatible components for operator-grade AI products.

Registry: `https://aurora.tootie.tv` · GitHub: `https://github.com/jmagar/aurora-design-system`

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": true,
  "tsx": true,
  "tailwind": {
    "css": "app/globals.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "registries": {
    "@aurora": "https://aurora.tootie.tv/r/{name}.json"
  }
}
```

Install Aurora:

```bash
cd apps/web
pnpm dlx shadcn@latest add https://aurora.tootie.tv/r/aurora-tokens.json
```

## Static export

`apps/web/out/.gitkeep` is tracked so Docker COPY paths exist, but generated files under `apps/web/out/*` are ignored. Build assets locally before embedding them in release builds.

See `docs/PATTERNS.md` §A3, §A4, §A5 for embedding, Aurora, and the web feature gate patterns.
