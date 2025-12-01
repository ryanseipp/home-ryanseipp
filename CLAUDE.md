# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Build Commands

This is a Nix-managed monorepo. Enter the development shell first:

```bash
nix develop
```

### Building Services

```bash
nix build .#<service-name>    # Build specific service (gateway, identity, testdotnet, testjava, email)
nix flake check               # Build and check all services
```

### Formatting

```bash
nix fmt                       # Format all code (uses treefmt with language-specific formatters)
```

### Per-Language Commands

**Rust** (gateway, identity):

```bash
cargo build -p <service>      # Build single service
cargo test -p <service>       # Test single service
cargo watch -x 'build -p <service>'  # Watch mode
```

**Deno** (email):

```bash
cd services/email
deno task dev                 # Run with watch mode
deno task test                # Run tests
deno task test:watch          # Watch tests
deno lint                     # Lint
deno fmt                      # Format
```

**Java/Quarkus** (testjava):

```bash
cd services/testjava
./gradlew build               # Build
./gradlew test                # Run tests
./gradlew quarkusDev          # Dev mode with hot reload
```

**Protobuf** (generates TypeScript for email service):

```bash
buf generate --template buf.gen.yaml proto
```

## Architecture

### Service Types

Services are declared in `flake.nix` and built via language-specific Nix modules
in `nix/`:

- **rustServices**: Built with Crane, workspace members defined in root
  `Cargo.toml`
- **dotnetServices**: Built with NativeAOT for standalone executables
- **javaServices**: Built with Quarkus + GraalVM native image
- **denoServices**: Built with Deno compile

All services target native compilation (no runtime dependencies) and produce OCI
images.

### Cross-Cutting Concerns

- **Observability**: All services must integrate OpenTelemetry SDK for
  traces/metrics via OTLP. Logs use structured JSON with trace correlation.
- **Inter-service communication**: Services use gRPC (protobufs in `proto/`).
  API Gateway aggregates; services publish to Kafka rather than calling each
  other directly.
- **Security**: mTLS for inter-service, secrets via environment or filesystem.

### Key Paths

- `proto/` - Protobuf definitions shared across services
- `nix/*.nix` - Build modules for each language (rust.nix, dotnet.nix, java.nix,
  deno.nix)
- `services/<name>/` - Individual service source code
