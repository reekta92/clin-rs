{
  description = "Feature-packed terminal note management app inspired by Obsidian";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "clin";
          version = "0.10.0-rc.2"; # This will be updated by the release workflow
          doCheck = false;

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

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

          meta = with pkgs.lib; {
            description = "Feature-packed terminal note management app inspired by Obsidian";
            homepage = "https://github.com/reekta92/clin-rs";
            license = licenses.gpl3;
            maintainers = [ ];
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            cargo
            rustc
            openssl
            zlib
            libgit2
            libx11
            libxcb
          ];
        };
      }
    );
}
