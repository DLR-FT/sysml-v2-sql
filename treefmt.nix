{ ... }:
{
  # Used to find the project root
  projectRootFile = "flake.nix";
  programs.nixfmt.enable = true;
  programs.prettier = {
    enable = true;
    includes = [
      "*.css"
      "*.html"
      "*.js"
      "*.json"
      "*.json5"
      "*.md"
      "*.mdx"
      "*.yaml"
      "*.yml"
    ];
  };
  programs.rustfmt.enable = true;
  programs.sql-formatter = {
    # TODO enable once https://github.com/numtide/treefmt-nix/issues/400 is fixed
    # enable = true;
    dialect = "sqlite";
  };
  programs.taplo.enable = true; # formats TOML files
}
