# Run smoke tests with a fresh VM.
#
# ```
# nix-build nix/vm-test-smoke.nix
# ```
#
# https://nixos.org/manual/nixos/stable/index.html#sec-nixos-tests

let
  nixpkgsSource = import ./nixpkgs-nixos-21.05.nix { };
in
import "${nixpkgsSource}/nixos/tests/make-test-python.nix" ({ pkgs, ... }:
  {
    name = "smoke";
    nodes = {
      server = { pkgs, ... }: {
        imports = [ ./tere-nixos-module.nix ];
        config = {
          virtualisation.graphics = false;
          services.tere.enable = true;
        };
      };
    };
    testScript = builtins.readFile ./vm-test-smoke.py;
  })
