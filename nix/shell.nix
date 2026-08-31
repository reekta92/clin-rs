{
  mkShell,
  pkg-config,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer,
  openssl,
  zlib,
  libgit2,
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

    openssl
    zlib
    libgit2
    libx11
    libxcb
  ];
}
