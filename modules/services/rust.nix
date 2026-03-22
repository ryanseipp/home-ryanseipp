# Rust service builder module
# Defines rustServices option and builds packages for each declared service
{
  lib,
  flake-parts-lib,
  inputs,
  self,
  ...
}:
let
  inherit (flake-parts-lib) mkPerSystemOption;
in
{
  options.perSystem = mkPerSystemOption (
    { ... }:
    {
      options.rustServices = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "List of Rust service names to build from services/ directory";
      };
      options.rustCraneLib = lib.mkOption {
        type = lib.types.unspecified;
        description = "Crane library instance configured with the Rust toolchain";
      };
    }
  );

  config.perSystem =
    { config, pkgs, ... }:
    let
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
        p:
        p.rust-bin.stable.latest.default.override {
          extensions = [ "llvm-tools-preview" ];
          targets = [
            "x86_64-unknown-linux-musl"
            "aarch64-unknown-linux-musl"
            "wasm32-unknown-unknown"
          ];
        }
      );

      src = lib.cleanSourceWith {
        src = self;
        filter =
          path: type:
          (craneLib.filterCargoSources path type)
          || (lib.hasSuffix ".proto" path)
          || (lib.hasInfix "/migrations/" path)
          || (lib.hasInfix "/.sqlx/" path)
          || (lib.hasSuffix ".html" path)
          || (lib.hasSuffix ".css" path)
          || (lib.hasSuffix ".js" path)
          || (lib.hasSuffix ".svg" path)
          || (lib.hasSuffix ".ico" path)
          || (lib.hasInfix "/public/" path)
          || (lib.hasInfix "/style/" path);
      };

      commonArgs = {
        inherit src;
        strictDeps = true;
        nativeBuildInputs = [
          pkgs.cmake
          pkgs.clang
          pkgs.git
          pkgs.perl
          pkgs.protobuf
        ];
        PROTOBUF_LOCATION = "${pkgs.protobuf}";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        PROTOC_INCLUDE = "${pkgs.protobuf}/include";
        SQLX_OFFLINE = "true";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      buildRustService =
        serviceName:
        let
          servicePath = "${self}/services/${serviceName}";
        in
        craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
            pname = serviceName;
            cargoExtraArgs = "-p ${serviceName}";
            # Use cleaned source from commonArgs; cargoExtraArgs selects the package
            version =
              (craneLib.crateNameFromCargoToml {
                cargoToml = "${servicePath}/Cargo.toml";
              }).version;
          }
        );

      workspaceClippy = craneLib.cargoClippy (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        }
      );

      workspaceDoc = craneLib.cargoDoc (
        commonArgs
        // {
          inherit cargoArtifacts;
          env.RUSTDOCFLAGS = "--deny warnings";
        }
      );

      workspaceNextest = craneLib.cargoNextest (
        commonArgs
        // {
          inherit cargoArtifacts;
          partitions = 1;
          partitionType = "count";
          cargoNextestPartitionsExtraArgs = "--no-tests=pass";
        }
      );

      workspaceCoverage = craneLib.cargoNextest (
        commonArgs
        // {
          inherit cargoArtifacts;
          withLlvmCov = true;
          partitions = 1;
          partitionType = "count";
          cargoNextestPartitionsExtraArgs = "--no-tests=pass";
        }
      );
    in
    {
      rustServices = [
        "gateway"
        "identity"
      ];

      rustCraneLib = craneLib;

      packages = lib.genAttrs config.rustServices buildRustService // {
        coverage = workspaceCoverage;
      };
      checks = {
        clippy = workspaceClippy;
        doc = workspaceDoc;
        nextest = workspaceNextest;
      };
    };
}
