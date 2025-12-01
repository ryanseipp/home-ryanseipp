# Java/Quarkus GraalVM native service builder module
# Defines javaServices option and builds packages for each declared service
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
      options.javaServices = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "List of Java/Quarkus service names to build from services/ directory";
      };
    }
  );

  config.perSystem =
    { config, pkgs, ... }:
    let
      buildJavaService =
        serviceName:
        let
          # Use relative path from this file to get a proper path type
          # ../services/X resolves to the services directory from nix/
          servicePath = ../services + "/${serviceName}";
          gradle25 = (
            pkgs.gradleFromWrapper {
              wrapperPropertiesPath = "${servicePath}/gradle/wrapper/gradle-wrapper.properties";
              defaultJava = pkgs.jdk25_headless;
            }
          );
        in
        (pkgs.buildGradleApplication {
          pname = serviceName;
          version = "0.1.0";
          src = servicePath;

          jdk = pkgs.jdk25_headless;
          gradle = gradle25;

          nativeBuildInputs =
            with pkgs;
            [
              graalvmPackages.graalvm-ce
              gcc
              zlib.dev
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.ICU
              pkgs.darwin.binutils
            ];

          buildTask = "-Dquarkus.native.enabled=true -Dquarkus.package.jar.enabled=false build -x test";

          meta = {
            description = "${serviceName} microservice";
          };
        }).overrideAttrs
          (oldAttrs: {
            # Quarkus native builds produce a single executable, not the standard Gradle app layout
            installPhase = ''
              runHook preInstall

              mkdir -p $out/bin
              cp build/*-runner $out/bin/${serviceName}
              chmod +x $out/bin/${serviceName}

              runHook postInstall
            '';
          });
    in
    {
      packages = lib.genAttrs config.javaServices buildJavaService;
    };
}
