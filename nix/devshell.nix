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
      allServiceNames = config.rustServices ++ config.dotnetServices;
      servicePackages = map (name: config.packages.${name}) allServiceNames;
    in
    {
      devShells.default = craneLib.devShell {
        checks = inputs.self.checks;

        inputsFrom = servicePackages;

        packages = with pkgs; [
          argocd
          cargo-watch
          cilium-cli
          clang
          dotnetCorePackages.sdk_10_0
          helmfile
          k9s
          kind
          kubectl
          kubectx
          kubernetes-helm
          kustomize
          opentofu
          yq-go
          zlib.dev
        ];
      };
    };
}
