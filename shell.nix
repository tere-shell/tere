let
  # Avoid pkgs.fetchFromGitHub because with that we'd need to import nixpkgs to construct nixpkgs, and that ends up putting nix into a recursion and aborting. This also means this .nix file won't take an optional `pkgs` argument like most of them do.
  rustOverlay = (import (builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/6fec958e1ca028e0a1b0edfff613ff9b5bcfe3d0.tar.gz";
    sha256 = "0crbz4jixhbxwkymr9znpmgx4ry7zkfpxqwnmaqkjl132k1mp8yz";
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
    mdbook
    (callPackage ./nix/mdbook-linkcheck.nix { })
    (callPackage ./nix/mdbook-graphviz.nix { })
    (callPackage ./nix/mdbook-mermaid.nix { })
  ];
}
