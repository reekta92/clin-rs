{
  mkShell,
  callPackage,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer,
}:
mkShell {
  name = "clin";

  inputsFrom = [ (callPackage ./package.nix { }) ];

  nativeBuildInputs = [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
  ];
}
