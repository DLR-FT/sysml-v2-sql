final: prev:
let
  inherit (prev) lib;

  # all packages from the local tree
  sysmlV2SqlPkgs = lib.filesystem.packagesFromDirectoryRecursive {
    inherit (final) callPackage;

    # local tree of packages
    directory = ./pkgs;
  };
in

{
  # custom namespace for packages from the local tree
  inherit sysmlV2SqlPkgs;
}
// sysmlV2SqlPkgs
