{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
}:

rustPlatform.buildRustPackage {
  pname = "clin-rs";
  version = (lib.importTOML ./Cargo.toml).package.version;
  src = ./.;

  cargoHash = "sha256-x+7xlo7ovstn28Z1xGDmrj9o2KoiucKL+BQ+dYJWbFI=";

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ openssl ];

  meta = {
    description = "TUI note management app, a terminal reimplementation of Obsidian";
    homepage = "https://github.com/reekta92/clin-rs";
    license = lib.licenses.gpl3Only;
    maintainers = [ ];
    mainProgram = "clin";
  };
}
