{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs =
    {
      self,
      nixpkgs,
      treefmt-nix,
      ...
    }@inputs:
    {
      overlays.default = import ./overlay.nix;
    }
    // (inputs.flake-utils.lib.eachSystem [ "x86_64-linux" "i686-linux" "aarch64-linux" ] (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        };
        pkgs-cross-x86_64-windows = import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
          crossSystem = {
            config = "x86_64-w64-mingw32"; # That's the triplet they use in the mingw-w64 docs.
            libc = "msvcrt"; # This distinguishes the mingw (non posix) toolchain
          };
        };

        # universal formatter
        treefmtEval = treefmt-nix.lib.evalModule pkgs ./treefmt.nix;
      in
      {
        # packages
        packages = pkgs.sysmlV2SqlPkgs // {
          sysml-v2-sql-static = pkgs.pkgsStatic.sysml-v2-sql;
          sysml-v2-sql-x86_64-windows = pkgs-cross-x86_64-windows.sysml-v2-sql;
          release-package = pkgs.release-package.override {
            inherit (self.packages.${system})
              sysml-v2-sql-static
              sysml-v2-sql-x86_64-windows
              ;
          };
        };

        # a devshell with all the necessary bells and whistles
        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.sysml-v2-sql ];
          nativeBuildInputs = with pkgs; [
            # Rust
            cargo-deny
            cargo-flamegraph
            cargo-llvm-cov
            cargo-release
            cargo-udeps
            clippy
            rust-analyzer
            rustfmt

            # Tools to explore SQLite databases
            datasette
            sqlite
            sqlpage

            # Dev
            inotify-tools
          ];
          env = {
            # required for cargo-llvm-cov to work
            inherit (pkgs.cargo-llvm-cov) LLVM_COV LLVM_PROFDATA;
          };
        };

        # for `nix fmt`
        formatter = treefmtEval.config.build.wrapper;

        # always check these
        checks = {
          formatting = treefmtEval.config.build.check self;
        };
      }
    ));
}
