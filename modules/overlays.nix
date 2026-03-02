# Configures pkgs with required overlays for all language modules
{ inputs, ... }:
{
  perSystem =
    { system, ... }:
    {
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;
        config.allowUnfreePredicate =
          pkg:
          builtins.elem (inputs.nixpkgs.lib.getName pkg) [
            "1password-cli"
          ];
        overlays = [
          (import inputs.rust-overlay)
          inputs.build-gradle-application.overlays.default
          inputs.nix-deno.overlays.default
        ];
      };
    };
}
