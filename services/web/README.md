# web

Leptos SSR web UI for home.ryanseipp.com, served by Axum.

## Architecture

The web service is a **client** of the API Gateway — not a microservice behind
it. It renders pages on the server (SSR) and hydrates them in the browser via
WebAssembly.

```
Browser ──HTTP──▶ Web (SSR + WASM hydration)
                      │
                      │ HTTPS + JSON (proxied /api/v1/*)
                      ▼
                  API Gateway ──mTLS/gRPC──▶ Identity, etc.
```

### Key design decisions

- **No `#[server]` functions.** We control the API surface and routing structure
  intentionally via the Gateway. The web service is just a client that happens
  to have a server component for SSR. A mobile app would be another client
  talking to the same Gateway.

- **No mTLS.** Communication between this service and the Gateway uses regular
  HTTPS+JSON. The mTLS boundary ends at the Gateway — it is the entrypoint for
  clients into the protected world.

- **API proxy.** All `/api/v1/*` requests from the browser are proxied through
  the SSR server to the Gateway. This avoids CORS, keeps the Gateway URL
  internal, and enables server-side trace propagation.

- **Accessible components.** Manually implemented WAI-ARIA compliant primitives
  inspired by shadcn/ui and Radix. Styled with TailwindCSS v4. No external
  component library (Radix-Leptos and Rust shadcn/ui are unmaintained).

## Configuration

All via environment variables with `WEB_` prefix and `__` separator:

| Variable            | Default             | Description          |
| ------------------- | ------------------- | -------------------- |
| `WEB__LISTEN_ADDR`  | `[::]:3000`         | HTTP listen address  |
| `WEB__GATEWAY__URL` | `http://[::1]:8080` | API Gateway base URL |
| `RUST_LOG`          | `info`              | Log level filter     |

OTel configuration uses standard `OTEL_*` environment variables.

## Development

```bash
nix develop              # Enter devshell (includes cargo-leptos, leptosfmt, tailwindcss)
cargo leptos watch       # Dev mode with SSR + WASM hot reload at localhost:3000
```

## Building

```bash
cargo leptos build --release    # Production build (dev)
nix build .#web                 # Hermetic Nix build
```

## Testing

```bash
cargo clippy -p web --features ssr       # Lint
cargo nextest run -p web --features ssr  # Unit tests

cd e2e
deno task test:install                   # Install Playwright browsers (first time)
deno task test                           # E2E + accessibility tests
```

E2E tests require running services: web, Gateway, Identity, Postgres, ScyllaDB.

## Component library

Components live in `src/components/` with a shadcn-inspired architecture:

- `Button` — variants (Primary, Secondary, Outline, Ghost, Destructive), sizes
- `Input` — with `aria-invalid`/`aria-describedby` error states
- `Label` — linked to inputs via `for` attribute
- `Card` / `CardHeader` / `CardContent` / `CardFooter` — composition
- `Alert` — `role="alert"` (error/warning) or `role="status"` (info/success)
- `cn()` utility — Tailwind class merging via `tailwind-fuse`

### Adding new components

1. Create `src/components/<name>.rs`
2. Re-export from `src/components/mod.rs`
3. Follow the pattern: `#[component]` function, enum variants for visual
   options, `cn()` for class merging, WAI-ARIA attributes baked in
