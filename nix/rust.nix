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
    }
  );

  config.perSystem =
    { config, pkgs, ... }:
    let
      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
        p:
        p.rust-bin.stable.latest.default.override {
          targets = [
            "x86_64-unknown-linux-musl"
            "aarch64-unknown-linux-musl"
          ];
        }
      );

      src = craneLib.cleanCargoSource self;

      commonArgs = {
        inherit src;
        strictDeps = true;
        nativeBuildInputs = [
          pkgs.git
          pkgs.protobuf
        ];
        PROTOBUF_LOCATION = "${pkgs.protobuf}";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        PROTOC_INCLUDE = "${pkgs.protobuf}/include";
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
    in
    {
      packages = lib.genAttrs config.rustServices buildRustService;
    };
}
