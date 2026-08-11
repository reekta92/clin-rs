{
  description = "Feature-packed terminal note management app inspired by Obsidian";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        clin = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
          pname = "clin";
          version = (pkgs.lib.importTOML ./Cargo.toml).package.version;

          doCheck = false;
          __structuredAttrs = true;

          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./src
              ./Cargo.toml
              ./Cargo.lock
            ];
          };

          cargoLock.lockFile = "${finalAttrs.src}/Cargo.lock";

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            openssl
            zlib
            libgit2
            libx11
            libxcb
          ];

          meta = {
            description = "Feature-packed terminal note management app inspired by Obsidian";
            homepage = "https://github.com/reekta92/clin-rs";
            license = pkgs.lib.licenses.gpl3;
            mainProgram = "clin";
          };
        });
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.clin;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
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
        };
      });
    };
}
