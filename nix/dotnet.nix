# .NET NativeAOT service builder module
# Defines dotnetServices option and builds packages for each declared service
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
      options.dotnetServices = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "List of .NET service names to build from services/ directory";
      };
    }
  );

  config.perSystem =
    {
      config,
      pkgs,
      system,
      ...
    }:
    let
      buildDotnetService =
        serviceName:
        let
          servicePath = self + "/services/${serviceName}";
          lockfile = "${servicePath}/packages.lock.json";
        in
        pkgs.buildDotnetModule {
          pname = serviceName;
          version = "0.1.0";
          src = servicePath;

          projectFile = "${serviceName}.csproj";
          nugetDeps = inputs.nuget-packageslock2nix.lib {
            inherit system;
            lockfiles = [ lockfile ];
            name = "${serviceName}-deps";
            # Exclude ILCompiler packages that are implicitly provided by dotnet-sdk for NativeAOT
            excludePackages = [
              "Microsoft.DotNet.ILCompiler-10.0.0-rc.2.25502.107"
              "Microsoft.NET.ILLink.Tasks-10.0.0-rc.2.25502.107"
            ];
          };

          dotnet-sdk = pkgs.dotnetCorePackages.sdk_10_0;
          dotnet-runtime = pkgs.dotnetCorePackages.sdk_10_0;

          nativeBuildInputs =
            with pkgs;
            [
              clang
              zlib.dev
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.ICU
              pkgs.darwin.binutils
            ];

          selfContainedBuild = true;
          executables = [ serviceName ];
          doCheck = false;
        };
    in
    {
      packages = lib.genAttrs config.dotnetServices buildDotnetService;
    };
}
