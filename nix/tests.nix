{
  lib,
  callPackage,
  rustPlatform,
  runCommand,
}:
let
  package = callPackage ./package.nix { };
  source = ./tests/source;
  checkSource =
    src:
    let
      overridden = package.overrideAttrs { inherit src; };
    in
    assert
      builtins.readFile overridden.cargoDeps.lockFile == builtins.readFile (source + "/Cargo.lock");
    true;
  dependencyOverride = package.overrideAttrs (
    finalAttrs: _: {
      src = source;
      cargoDeps = rustPlatform.importCargoLock {
        lockFile = finalAttrs.src + "/Cargo.lock";
      };
    }
  );
in
assert builtins.readFile package.cargoDeps.lockFile == builtins.readFile ../Cargo.lock;
assert builtins.isString package.cargoDeps.drvPath;
assert builtins.isString dependencyOverride.cargoDeps.drvPath;
assert checkSource source;
assert checkSource (toString source);
assert checkSource { outPath = toString source; };
assert checkSource (lib.cleanSource source);
assert checkSource (
  lib.fileset.toSource {
    root = source;
    fileset = source;
  }
);
runCommand "clin-source-overrides" { } ''
  touch "$out"
''
