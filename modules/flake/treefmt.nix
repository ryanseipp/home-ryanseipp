{ inputs, ... }:
{
  imports = [ inputs.treefmt-nix.flakeModule ];

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
          leptosfmt = {
            enable = true;
            includes = [ "services/web/**/*.rs" ];
          };
          nixfmt.enable = true;
          rustfmt.enable = true;
          hclfmt.enable = true;
          terraform.enable = true;
          # csharpier.enable = true;
          google-java-format.enable = true;
          ktfmt.enable = true;
          prettier = {
            enable = true;
            includes = [
              "*.md"
              "*.json"
              "*.js"
              "*.ts"
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
