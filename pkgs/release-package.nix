{
  lib,
  system,
  path,
  runCommandNoCC,
  libarchive,
}:
let
  ourPkgs = {
    x86_64-linux = {
      pkgsTarget =
        (import path {
          inherit system;
          overlays = [ (import ../overlay.nix) ];
          crossSystem.config = "x86_64-linux";
        }).pkgsStatic;
      archiveType = "tar.gz";
    };

    i686-linux = {
      pkgsTarget =
        (import path {
          inherit system;
          overlays = [ (import ../overlay.nix) ];
          crossSystem.config = "i686-linux";
        }).pkgsStatic;
      archiveType = "tar.gz";
    };

    aarch64-linux = {
      pkgsTarget =
        (import path {
          inherit system;
          overlays = [ (import ../overlay.nix) ];
          crossSystem.config = "aarch64-linux";
        }).pkgsStatic;
      archiveType = "tar.gz";
    };

    x86_64-windows = {
      pkgsTarget = import path {
        inherit system;
        overlays = [ (import ../overlay.nix) ];
        crossSystem = {
          config = "x86_64-w64-mingw32"; # That's the triplet they use in the mingw-w64 docs.
          libc = "msvcrt"; # This distinguishes the mingw (non posix) toolchain
        };
      };
      archiveType = "zip";
    };
  };
in
runCommandNoCC "lace-result"
  {
    nativeBuildInputs = [ libarchive ];
    passthru = { inherit ourPkgs; };
  }
  ''
    mkdir -- $out

    ${lib.strings.concatStringsSep "\n" (
      lib.attrsets.mapAttrsToList (
        name:
        { pkgsTarget, archiveType }:
        ''
          cp --dereference --recursive -- ${pkgsTarget.sysml-v2-sql}/bin/ ${name}
          chmod --recursive u+rwx -- ${name}
          bsdtar --auto-compress --create --file $out/${name}.${archiveType} -- ${name}
        ''
      ) ourPkgs
    )}

  ''
