# Code formatting configuration with treefmt-nix
{ ... }:
{
  perSystem =
    { pkgs, ... }:
    {
      treefmt = {
        settings.global.excludes = [
          ".envrc"
          "LICENSE"
          "*.gitignore"
          "*.gitkeep"
          "target/"
          "node_modules/"
        ];
        programs = {
          deno.enable = true;
          nixfmt.enable = true;
          rustfmt.enable = true;
          terraform.enable = true;
          csharpier.enable = true;
          google-java-format.enable = true;
          ktfmt.enable = true;
          prettier = {
            enable = true;
            includes = [
              "*.md"
              "*.json"
              "*.yaml"
              "*.yml"
            ];
            settings.proseWrap = "always";
          };
          taplo = {
            enable = true;
            settings = {
              include = [
                "*.toml"
                "Cargo.lock"
              ];
              formatting.array_auto_expand = false;
            };
          };
          sql-formatter = {
            enable = true;
            dialect = "postgresql";
          };
        };
      };
    };
}
