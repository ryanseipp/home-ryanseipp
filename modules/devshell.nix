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
      # Collect all service packages for inputsFrom
      allServiceNames =
        config.rustServices ++ config.dotnetServices ++ config.javaServices ++ config.denoServices;
      servicePackages = map (name: config.packages.${name}) allServiceNames;
    in
    {
      devShells.default = config.rustCraneLib.devShell {
        checks = inputs.self.checks;

        inputsFrom = servicePackages;

        PROTOBUF_LOCATION = "${pkgs.protobuf}";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        PROTOC_INCLUDE = "${pkgs.protobuf}/include";

        packages = with pkgs; [
          _1password-cli
          age
          argocd
          buf
          protobuf
          cargo-llvm-cov
          cargo-nextest
          cargo-watch
          sqlx-cli
          cilium-cli
          clang
          corepack_24
          deno
          dotnetCorePackages.sdk_10_0
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
          sops
          talhelper
          talosctl
          terragrunt
          yq-go
          zlib.dev
        ];
      };
    };
}
