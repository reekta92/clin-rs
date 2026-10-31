{
  lib,
  stdenv,
  rustPlatform,
  makeBinaryWrapper,
  pkg-config,
  zlib,
  libx11,
  libxcb,

  wl-clipboard,
  tesseract,

  waylandSupport ? stdenv.hostPlatform.isLinux,
  ocrSupport ? false,
}:
let
  runtimeDeps = lib.optional waylandSupport wl-clipboard ++ lib.optional ocrSupport tesseract;
  doWrap = runtimeDeps != [ ];
in
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
        (root + /build.rs)
        (root + /clin.ico)
        (root + /assets)
      ];
    };

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "pinstar-1.1.4" = "sha256-71GFX1PDjv10v9Fz7CanR++zyoQ0gQiL3z9//HrI/Ls=";
    };
  };

  nativeBuildInputs = [
    pkg-config
  ]
  ++ lib.optional doWrap makeBinaryWrapper;

  buildInputs = [
    zlib
  ]
  ++ lib.optionals stdenv.hostPlatform.isLinux [
    libx11
    libxcb
  ];

  postInstall = ''
    install -Dm444 assets/clin.desktop -t $out/share/applications
    install -Dm444 assets/icon.svg $out/share/icons/hicolor/scalable/apps/clin.svg
    install -Dm444 assets/clin.png -t $out/share/icons/hicolor/256x256/apps
  ''
  + lib.optionalString stdenv.hostPlatform.isDarwin ''
    app=$out/Applications/clin.app
    mkdir -p $out/Applications
    cp -R assets/clin.app "$app"
    mkdir -p "$app/Contents/Resources"
    ln -s $out/bin/clin "$app/Contents/Resources/clin"
    cp assets/clin.icns "$app/Contents/Resources/clin.icns"
    substituteInPlace "$app/Contents/Info.plist" \
      --replace-fail '<string>0.0.0</string>' '<string>${lib.head (lib.splitString "-" finalAttrs.version)}</string>'
  '';

  postFixup = lib.optionalString doWrap ''
    wrapProgram $out/bin/clin \
      --prefix PATH : ${lib.makeBinPath runtimeDeps}
  '';

  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
    test "$($out/bin/clin --version)" = "clin ${finalAttrs.version}"
    $out/bin/clin --help > /dev/null
    test -f $out/share/applications/clin.desktop
    test -f $out/share/icons/hicolor/scalable/apps/clin.svg
    test -f $out/share/icons/hicolor/256x256/apps/clin.png
  ''
  + lib.optionalString stdenv.hostPlatform.isDarwin ''
    test -x $out/Applications/clin.app/Contents/MacOS/clin
    test -f $out/Applications/clin.app/Contents/Resources/clin.icns
    test "$($out/Applications/clin.app/Contents/Resources/clin --version)" = "clin ${finalAttrs.version}"
  ''
  + ''
    runHook postInstallCheck
  '';

  meta = {
    description = "Feature-packed terminal note management app inspired by Obsidian";
    homepage = "https://github.com/reekta92/clin-rs";
    license = lib.licenses.gpl3Only;
    mainProgram = "clin";
  };
})
