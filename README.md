# home.ryanseipp.com

An internal website for personal use, and primarily to test out new programming
concepts & languages. I'm both a practitioner and operator, so the idea is to
build the software environment I wish I could use day-to-day at work, and keep
learning new practices.

## Cross-cutting concerns

### Observability

All components must be natively observable via OpenTelemetry. This means
integrating with the OpenTelemetry SDK in the language of choice to emit metrics
and traces via OTLP. Due to the nature of logs and the need to support telemetry
collection for third-party tools like DBs, logs will be emitted in a structured
JSON format with traceId and spanIds attached for correlation between signals.

### Security

All components must be secured to a high standard. This includes securing
inter-service communication via mTLS, communicating with third-party tools only
over encrypted channels using least-privilege authentication, implementing
authorization and rate-limiting, and more. Any secrets required by the
application to function must be passed via the environment, or even preferably
files on the filesystem.

### Isolation

Services must be properly isolated so as to be loosely coupled. This means the
web app will call an API Gateway which will aggregate data across multiple
services. However, services may not additionally call other services, unless
absolutely required, as this introduces performance and reliability concerns.
Any data needed across multiple services will be published over a message topic
in Kafka for consumption.

Services will each own their own data stores, in whichever form is needed. Data
within the system will be eventually consistent, prioritizing high availability
and fault tolerance.

### Builds

Builds must be reproducible, and will be managed via Nix. The end artifact of a
build will most commonly be an OCI image that is signed, attested with
provenance, and has an available SBOM.

## Development

### Getting Started

This monorepo supports Rust, .NET, Java/Quarkus, and Deno services. To start developing:

```bash
# Enter the Nix development environment
nix develop

# The dev shell includes:
# - Rust toolchain with cargo, rustc
# - .NET 10 SDK
# - JDK 25 + Gradle for Quarkus services
# - Deno runtime
# - GraalVM for native image compilation
# - Native toolchain (clang, zlib) for NativeAOT
# - Infrastructure tools (kubectl, helm, etc.)
```

### Creating a New Service

#### Rust Services

Rust services use Cargo workspaces and are managed via Crane:

1. Create your service in `services/`:

   ```bash
   cd services
   cargo new my-service
   ```

2. Add to workspace in root `Cargo.toml`:

   ```toml
   members = ["services/my-service"]
   ```

3. Add to `flake.nix`:
   ```nix
   rustServices = [ "my-service" ];
   ```

#### .NET Services

.NET services are built with NativeAOT for standalone executables:

1. Create your service in `services/`:

   ```bash
   cd services
   dotnet new web -n my-service
   ```

2. Configure your `.csproj` with required properties:

   ```xml
   <PropertyGroup>
     <TargetFramework>net10.0</TargetFramework>
     <PublishAot>true</PublishAot>
     <RestorePackagesWithLockFile>true</RestorePackagesWithLockFile>
     <InvariantGlobalization>true</InvariantGlobalization>
   </PropertyGroup>
   ```

   | Property                       | Purpose                                        |
   | ------------------------------ | ---------------------------------------------- |
   | `PublishAot`                   | Enables NativeAOT compilation                  |
   | `RestorePackagesWithLockFile`  | Generates `packages.lock.json` required by Nix |
   | `InvariantGlobalization`       | Avoids ICU dependency issues in NativeAOT      |

3. Generate the lock file:

   ```bash
   dotnet restore
   ```

   This creates `packages.lock.json` which must be committed to the repository.

4. Add to `flake.nix`:
   ```nix
   dotnetServices = [ "my-service" ];
   ```

   Note: The `.csproj` filename must match the service directory name (e.g.,
   `services/my-service/my-service.csproj`).

#### Java/Quarkus Services

Java services use Quarkus with GraalVM native image compilation:

1. Create your service in `services/`:

   ```bash
   cd services
   quarkus create app my-service --gradle-kotlin-dsl
   ```

2. Configure `build.gradle.kts` for Java 25:

   ```kotlin
   java {
     sourceCompatibility = JavaVersion.VERSION_25
     targetCompatibility = JavaVersion.VERSION_25
   }
   ```

3. Add Gradle wrapper with SHA256 checksum:

   ```bash
   gradle wrapper
   ```

   Edit `gradle/wrapper/gradle-wrapper.properties` to include
   `distributionSha256Sum` (find checksums at https://gradle.org/release-checksums/).

4. Add to `flake.nix`:
   ```nix
   javaServices = [ "my-service" ];
   ```

   Note: The `rootProject.name` in `settings.gradle.kts` must match the service
   directory name. Nix builds with GraalVM native image and expects the Quarkus
   `build/*-runner` output.

#### Deno Services

Deno services are compiled to standalone executables:

1. Create your service in `services/`:

   ```bash
   cd services
   mkdir my-service && cd my-service
   deno init
   ```

2. Configure `deno.json` with required tasks:

   ```json
   {
     "tasks": {
       "compile": "deno compile --output my-service src/index.ts",
       "test": "deno test --allow-all"
     }
   }
   ```

   | Task      | Purpose                                            |
   | --------- | -------------------------------------------------- |
   | `compile` | Compiles to standalone executable (output must match service name) |
   | `test`    | Runs tests during Nix build                        |

3. Generate the lock file:

   ```bash
   deno install
   ```

   This creates `deno.lock` which must be committed to the repository.

4. Add to `flake.nix`:
   ```nix
   denoServices = [ "my-service" ];
   ```

   Note: The compiled output filename must match the service directory name.
   Nix expects the binary at `services/my-service/my-service`.
