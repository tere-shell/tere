let
  # Avoid pkgs.fetchFromGitHub because with that we'd need to import nixpkgs to construct nixpkgs, and that ends up putting nix into a recursion and aborting. This also means this .nix file won't take an optional `pkgs` argument like most of them do.
  rustOverlay = (import (builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/6fec958e1ca028e0a1b0edfff613ff9b5bcfe3d0.tar.gz";
    sha256 = "0crbz4jixhbxwkymr9znpmgx4ry7zkfpxqwnmaqkjl132k1mp8yz";
  }));
  rustVersionOverlay = (self: super:
    let
      rustChannel = super.rust-bin.fromRustupToolchainFile ./../rust-toolchain.toml;
    in
    {
      rustc = rustChannel;
      cargo = rustChannel;
    }
  );
in
# See nixpkgs.lib.fixedPoints.composeExtensions for inspiration.
  # Not using it because, as above, we'd need to import nixpkgs to define nixpkgs.
self: super:
let
  rustOverlayResult = rustOverlay self super;
  super2 = super // rustOverlayResult;
  versionOverlayResult = rustVersionOverlay self super2;
in
# Now combine the two attribute sets.
rustOverlayResult // versionOverlayResult
