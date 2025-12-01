# Development shell configuration
{
  lib,
  inputs,
  ...
}:
{
  perSystem =
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

      # Collect all service packages for inputsFrom
      allServiceNames =
        config.rustServices ++ config.dotnetServices ++ config.javaServices ++ config.denoServices;
      servicePackages = map (name: config.packages.${name}) allServiceNames;
    in
    {
      devShells.default = craneLib.devShell {
        checks = inputs.self.checks;

        inputsFrom = servicePackages;

        packages = with pkgs; [
          argocd
          buf
          cargo-watch
          cilium-cli
          clang
          corepack_24
          deno
          dotnetCorePackages.sdk_10_0
          gcc
          graalvmPackages.graalvm-ce
          helmfile
          jdk25
          k9s
          kind
          kubectl
          kubectx
          kubernetes-helm
          kustomize
          opentofu
          quarkus
          yq-go
          zlib.dev
        ];
      };
    };
}
