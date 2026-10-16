{
  mkShell,
  pkg-config,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer,
  zlib,
  libx11,
  libxcb,
}:
mkShell {
  name = "clin";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer

    zlib
    libx11
    libxcb
  ];
}
