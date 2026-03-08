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
cargo watch -x 'build -p <service>'  # Watch mode
```

**Rust Testing** (always use llvm-cov + nextest with db-tests):

```bash
cargo llvm-cov nextest -p identity --features db-tests  # Unit + integration tests with coverage (preferred)
cargo nextest run -p identity --features db-tests        # Unit + integration tests without coverage
```

This runs all tests (unit + integration) in a single compilation pass with
coverage. Requires `docker compose up -d postgres` for integration tests.

**SQLx Offline Cache** (after adding/changing SQL queries):

```bash
# Requires running postgres: docker compose up -d postgres
DATABASE_URL="postgres://identity:identity@localhost:5432/identity" sqlx database drop -y
DATABASE_URL="postgres://identity:identity@localhost:5432/identity" sqlx database create
DATABASE_URL="postgres://identity:identity@localhost:5432/identity" sqlx migrate run --source services/identity/migrations
DATABASE_URL="postgres://identity:identity@localhost:5432/identity" cargo sqlx prepare --workspace -- --all-targets --features db-tests
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

### Infrastructure (infra/)

**Terragrunt/OpenTofu** (network):

```bash
cd infra/network/<device>               # e.g., spine1, spine2, leaf1
export MIKROTIK_USERNAME=$(op item get RouterOS-Admin --vault Dev --field username)
export MIKROTIK_PASSWORD=$(op item get RouterOS-Admin --vault Dev --field password --reveal)
terragrunt init
terragrunt plan
terragrunt apply
terragrunt validate                     # Validate configuration
```

Network infrastructure uses Terragrunt + OpenTofu with
`terraform-routeros/routeros` provider to configure Mikrotik CCR2004 routers in
a spine-leaf topology with eBGP underlay over IPv6. Fabric topology is defined
in `locals.hcl`. The `modules/mikrotik` module configures per-device: identity,
loopbacks, P2P IPs, BFD, BGP peers, VLANs, firewall rules, DHCPv4/v6, and
certificates.

Each device (spine1, spine2, leaf1) has its own `terragrunt.hcl` referencing the
shared module and topology. Credentials via environment variables or 1Password.

**Talos** (nodes):

```bash
cd infra/nodes
talhelper gensecret > talsecret.sops.yaml  # Generate secrets (encrypt with SOPS)
talhelper genconfig                         # Generate Talos configs from talconfig.yaml
talosctl apply-config -n <node> -f <config> # Apply to nodes
```

**Helmfile** (per-cluster):

```bash
cd infra/clusters/<cluster>/infra  # or o11y, apps
helmfile template -f helmfile.yaml --output-dir rendered/  # Render and commit
```

**SOPS** (secrets):

```bash
sops -e secrets.yaml > secrets.sops.yaml    # Encrypt
sops -d secrets.sops.yaml                   # Decrypt
```

## Code Quality

- **No broken windows.** If a test is flaky, fix it. If you encounter a
  pre-existing issue while working nearby, fix it. Do not skip, ignore, or work
  around known problems.

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
- `infra/network/` - Mikrotik RouterOS Terraform configs
- `infra/nodes/` - Talos Linux cluster configs via talhelper
- `infra/clusters/<name>/` - Per-cluster ArgoCD manifests (infra, o11y, apps)
