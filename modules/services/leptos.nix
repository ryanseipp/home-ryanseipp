# Leptos service builder module
# Builds SSR + WASM hydration services using cargo-leptos
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
      options.leptosServices = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "List of Leptos service names to build from services/ directory";
      };
    }
  );

  config.perSystem =
    { config, pkgs, ... }:
    let
      craneLib = config.rustCraneLib;

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
          pkgs.cargo-leptos
          pkgs.wasm-bindgen-cli
          pkgs.tailwindcss_4
          pkgs.binaryen
        ];
        PROTOBUF_LOCATION = "${pkgs.protobuf}";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        PROTOC_INCLUDE = "${pkgs.protobuf}/include";
        SQLX_OFFLINE = "true";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      buildLeptosService =
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
            # web uses version.workspace = true; read from the workspace root
            # to avoid Crane's placeholder warning on inherited versions.
            version =
              (craneLib.crateNameFromCargoToml {
                cargoToml = "${self}/Cargo.toml";
              }).version;

            # cargo-leptos handles the dual-target build (server + WASM client).
            # It doesn't produce cargo's --message-format json log, so we
            # disable Crane's default install hook and handle it ourselves.
            doNotPostBuildInstallCargoBinaries = true;
            buildPhaseCargoCommand = ''
              cargo leptos build --release -p ${serviceName}
            '';
            installPhaseCommand = ''
              mkdir -p $out/bin $out/site
              cp target/release/${serviceName} $out/bin/
              cp -r target/site/. $out/site/
            '';
          }
        );
    in
    {
      leptosServices = [
        "web"
      ];

      packages = lib.genAttrs config.leptosServices buildLeptosService;
    };
}
