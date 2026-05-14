# apps/web

Operator dashboard and interactive tool runner for the MCP server. Built with Next.js 16 (static export), React 19, Tailwind CSS 4, Biome, and the Aurora design system.

## What it is

A static web UI served by the Rust binary alongside the MCP API. Three pages:

- **Dashboard** (`/`) — Server health (10s polling), status cards, quick action buttons, activity feed
- **Tool Runner** (`/tools/`) — Select an action, fill in parameters, see the request preview and live JSON response
- **API Explorer** (`/api/`) — Endpoint reference, surface parity table (MCP / REST / CLI), cURL examples for every action

## Stack

| Layer | Choice |
|---|---|
| Framework | Next.js 16 (App Router, static export) |
| Runtime | React 19 |
| Language | TypeScript 5 (strict) |
| Styles | Tailwind CSS 4 + Aurora design tokens |
| Components | shadcn/ui scaffolding over Radix UI primitives |
| Icons | lucide-react |
| Fonts | Manrope (display), Inter (sans), JetBrains Mono (mono) |

## Dev commands

```bash
pnpm dev        # dev server at http://localhost:3000
pnpm build      # static export -> out/
pnpm start      # start the Next.js production server (serves .next/)
pnpm lint       # Biome lint
pnpm check      # Biome lint + format check
```

## How it connects to the backend

All API calls go through `lib/api.ts`. The base URL is empty (relative) — the Rust server serves both the static files and the API from the same origin, so no CORS configuration is needed.

Every action is dispatched as:

```
POST /v1/example
{ "action": "<action>", "params": { ... } }
```

Helper functions (`greet`, `echo`, `status`, `callAction`) wrap the fetch call with typed `ApiResponse<T>` returns. Health and status use `GET /health` and `GET /status`.

## Design system (Aurora)

Dark mode is forced (`<html className="dark">`). All colors are CSS custom properties — never hardcoded hex values.

**Token layers** (defined in `components/aurora.css`):

| Category | Examples |
|---|---|
| Surfaces | `--aurora-page-bg`, `--aurora-panel-medium`, `--aurora-control-surface` |
| Borders | `--aurora-border-default`, `--aurora-border-strong` |
| Text | `--aurora-text-primary`, `--aurora-text-muted` |
| Accents | `--aurora-accent-*` (cyan), `--aurora-accent-pink*` (rose) |
| Status | `--aurora-success`, `--aurora-warn`, `--aurora-error`, `--aurora-info` |
| Radii | `--aurora-radius-1` (14px), `--aurora-radius-2` (18px), `--aurora-radius-3` (22px) |

Aurora tokens are bridged to shadcn's `--primary`, `--card`, `--destructive` aliases in `globals.css`.

**Adding a component:**

```bash
pnpm dlx shadcn@latest add @aurora/aurora-dialog
pnpm dlx shadcn@latest add @aurora/aurora-data-table
```

Components land in `components/ui/`. Use CVA (`class-variance-authority`) for variants and `cn()` from `lib/utils.ts` for className construction.

## File structure

```
apps/web/
├── app/
│   ├── layout.tsx        # Root layout — nav, forced dark mode, font variables
│   ├── page.tsx          # Dashboard (client component, polling)
│   ├── tools/page.tsx    # Tool Runner
│   ├── api/page.tsx      # API Explorer (static)
│   └── globals.css       # Tailwind import + Aurora token bridge + @theme
├── components/
│   ├── aurora.css        # Aurora token definitions (dark + light)
│   └── ui/               # Aurora/shadcn components
├── lib/
│   ├── api.ts            # Typed REST client
│   └── utils.ts          # cn() helper
├── components.json       # shadcn config — @aurora registry
├── next.config.ts        # Static export, output: "export"
└── tsconfig.json         # Path aliases (@/* → ./*), strict mode
```

## Constraints

- **Static export only** — no server actions, API routes, or streaming. `output: "export"` in `next.config.ts`.
- **Client components only** for interactive pages — use `"use client"` and React hooks.
- **All colors via CSS custom properties** — never write a raw hex value in a component.
- **All API calls through `lib/api.ts`** — don't fetch directly in components.
- **`cn()` for classNames** — never string concatenation.

## TEMPLATE

When adapting for a real service:

1. Update the action list in `tools/page.tsx` to match your service's actions.
2. Update the parameter forms for each action.
3. Update the action reference cards in `api/page.tsx`.
4. Replace `"example"` tool name references throughout with your service name.
5. Update `lib/api.ts` helper functions to match your service's actions and response shapes.
