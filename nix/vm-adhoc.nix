# Run an ad hoc virtual machine for exploration.
#
# ```
# nix build -f nix/vm-adhoc.nix && rm -f nixos.qcow2 && ./result/bin/run-nixos-vm
# ```

let
  nixpkgsSource = import ./nixpkgs-nixos-21.05.nix { };
  nixos = import "${nixpkgsSource}/nixos" {
    configuration =
      { config, pkgs, ... }:
      {
        imports = [ ./tere-nixos-module.nix ];
        config = {
          services.tere.enable = true;
          virtualisation.graphics = false;
          users = {
            # don't even prompt for the passphrase
            users."root".hashedPassword = "";
            users."testuser" = {
              isNormalUser = true;
              password = "testpassword";
            };
          };
          services.getty.autologinUser = "root";
          environment.systemPackages = with pkgs; [
            # example use: `socat - UNIX-CONNECT:/run/tere/socket/pty.socket,type=5`
            socat
          ];
        };
      };
  };
in
nixos.vm
