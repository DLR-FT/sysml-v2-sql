{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  sqlite,
}:

let
  filteredSrc =
    let
      # File suffices to include
      extensions = [
        "lock"
        "rs"
        "toml"
      ];

      # Files to explicitly include
      files = [
        "assets/schema.sql"
        "tests/example-dump.json"
      ];

      src = ../.;
      filter =
        path: type:
        let
          inherit (lib)
            any
            id
            removePrefix
            hasSuffix
            ;
          anyof = (any id);

          basename = baseNameOf (toString path);
          relative = removePrefix (toString src + "/") (toString path);
        in
        anyof [
          (type == "directory")
          (any (ext: hasSuffix ".${ext}" basename) extensions)
          (any (file: file == relative) files)
        ];
    in
    lib.sources.cleanSourceWith { inherit src filter; };

  cargoToml = lib.trivial.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {

  inherit (cargoToml.package) name version;

  src = filteredSrc;

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [
    openssl
    sqlite
  ];
  buildNoDefaultFeatures = true; # we don't want rusqlite's bundled sqlite
  buildFeatures = [ "native-tls" ]; # use native openssl
  cargoLock = {
    lockFile = ../Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  meta = {
    license = with lib.licenses; [
      asl20 # or
      mit
    ];
    maintainers = [ lib.maintainers.wucke13 ];
    mainProgram = cargoToml.package.name;
  };
}
