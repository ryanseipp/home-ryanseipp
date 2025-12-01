# Deno service builder module
# Defines denoServices option and builds packages for each declared service
{
  lib,
  flake-parts-lib,
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
      options.denoServices = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "List of Deno service names to build from services/ directory";
      };
    }
  );

  config.perSystem =
    { config, pkgs, ... }:
    let
      buildDenoService =
        serviceName:
        let
          servicePath = self + "/services/${serviceName}";
        in
        pkgs.denoPlatform.mkDenoDerivation {
          name = serviceName;
          version = "0.1.0";

          # Use full source - the build needs proto/ and buf.gen.yaml anyway
          src = self;

          lockFile = "${self}/services/${serviceName}/deno.lock";
          configFile = "${self}/services/${serviceName}/deno.json";

          nativeBuildInputs = [ pkgs.buf ];

          postUnpack = ''
            # Set HOME for buf (use deterministic path)
            export HOME=$NIX_BUILD_TOP

            cd $sourceRoot/services/${serviceName}
            deno task proto:generate
            cd -
          '';

          buildPhase = ''
            runHook preBuild

            cd services/${serviceName}
            deno task compile
            cd ../..

            runHook postBuild
          '';

          doCheck = true;

          checkPhase = ''
            runHook preCheck

            cd services/${serviceName}
            deno task test
            cd ../..

            runHook postCheck
          '';

          installPhase = ''
            runHook preInstall

            mkdir -p $out/bin
            cp services/${serviceName}/${serviceName} $out/bin/

            runHook postInstall
          '';

          meta = {
            description = "${serviceName} microservice (Deno)";
          };
        };
    in
    {
      packages = lib.genAttrs config.denoServices buildDenoService;
      # Deno services also serve as checks (doCheck = true runs tests)
      checks = lib.genAttrs config.denoServices buildDenoService;
    };
}
