{
  lib,
  runCommandNoCC,
  libarchive,

  # these must be passed for the release package to function
  sysml-v2-sql-x86_64-windows ? null,
  sysml-v2-sql-static ? null,
}:

runCommandNoCC "lace-result" { nativeBuildInputs = [ libarchive ]; } ''
  mkdir -- $out

  # lace windows package
  pushd ${sysml-v2-sql-x86_64-windows}/bin/
  bsdtar --auto-compress --create --file "$out/windows.zip" -H -- *
  popd

  # copy statically compiled linux binary
  cp ${lib.meta.getExe sysml-v2-sql-static} $out/
''
