{ config, lib, pkgs, ... }:
with lib;
let
  cfg = config.services.tere;

  tere = pkgs.callPackage ../default.nix { pkgsPath = pkgs.path; };

  parseSysusersToGroups = file:
    with builtins;
    let
      text = readFile file;
      # isString filters out the capture groups that split returns
      rawLines = filter isString (split "\n" text);
      goodLine = s: builtins.all (b: b) [
        (s != "")
        (match "#.*" s == null)
      ];
      lines = filter goodLine rawLines;
      parseGroup = line:
        let
          # https://www.freedesktop.org/software/systemd/man/sysusers.d.html
          # "g foo", no gid
          matches = match "g[[:space:]]+([^[:space:]]+)" line;
          group = head matches;
        in
        assert pkgs.lib.asserts.assertMsg (group != null)
          "sysusers.d contains something else than a group line: ${line}";
        group;
      groups = map parseGroup lines;
    in
    listToAttrs (map (group: { name = group; value = { }; }) groups);

in
{
  options = {
    services.tere = {
      enable = mkOption {
        type = types.bool;
        default = false;
        example = "true";
        description = "Whether to enable the Tere remote shell service, which allows secure remote logins.";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = with pkgs; [
      tere
    ];
    systemd.packages = [
      tere
    ];
    services.dbus.packages = [ tere ];
    # It's stupid that we have to repeat parts of the shipped unit files, for NixOS.
    # We could try parsing the unit files, to keep a single authoritative source.
    # This really belongs in NixOS.
    systemd.units."tere-pty.socket".wantedBy = [ "sockets.target" ];
    # Reimplement sysusers here to allow `users.mutableUsers=true`.
    # This really belongs in NixOS.
    users.groups = parseSysusersToGroups ./../server/systemd/lib/sysusers.d/50-tere.conf;
    # TODO polkit?
  };
}
