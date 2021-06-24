let
  rustOverlay = import ./nix/rust-overlay.nix;
  pkgs = import <nixpkgs> {
    overlays = [
      rustOverlay
    ];
  };
in
with pkgs;
stdenv.mkDerivation {
  name = "tere-shell";
  buildInputs = [
    rustc
    cargo
    cargo-edit
    cargo-watch
    mdbook
    (callPackage ./nix/mdbook-linkcheck.nix { })
    (callPackage ./nix/mdbook-graphviz.nix { })
    (callPackage ./nix/mdbook-mermaid.nix { })
  ];
}
