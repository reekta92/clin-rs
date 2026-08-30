{
  lib,
  rustPlatform,
  pkg-config,
  openssl,
  zlib,
  libgit2,
  libx11,
  libxcb,
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "clin";
  version = (lib.importTOML ../Cargo.toml).package.version;

  doCheck = false;
  __structuredAttrs = true;

  src =
    let
      root = ../.;
    in
    lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        (root + /src)
        (root + /Cargo.toml)
        (root + /Cargo.lock)
        (root + /assets/clin.desktop)
        (root + /assets/clin.png)
      ];
    };

  cargoLock.lockFile = "${finalAttrs.src}/Cargo.lock";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
    zlib
    libgit2
    libx11
    libxcb
  ];

  postInstall = ''
    install -Dm444 assets/clin.desktop -t $out/share/applications
    install -Dm444 assets/clin.png -t $out/share/icons/hicolor/256x256/apps
  '';

  meta = {
    description = "Feature-packed terminal note management app inspired by Obsidian";
    homepage = "https://github.com/reekta92/clin-rs";
    license = lib.licenses.gpl3;
    mainProgram = "clin";
  };
})
