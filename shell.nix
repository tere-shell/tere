let
  # Avoid pkgs.fetchFromGitHub because with that we'd need to import nixpkgs to construct nixpkgs, and that ends up putting nix into a recursion and aborting. This also means this .nix file won't take an optional `pkgs` argument like most of them do.
  rustOverlay = (import (builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/e88036b9fc7b6ad4e2db86944204877b9090d8b9.tar.gz";
    sha256 = "1m29m49f7q0r6qvzjxkyq3yqiqff6b4cwl385cbpz551421bmr63";
  }));
  pkgs = import <nixpkgs> {
    overlays = [ rustOverlay ];
  };
  rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
in
with pkgs;
stdenv.mkDerivation {
  name = "tere-shell";
  buildInputs = [
    rust
    cargo-edit
    cargo-watch
  ];
}
