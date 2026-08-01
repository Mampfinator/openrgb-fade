{
  config,
  lib,
  pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz") { },
  ...
}:
let 
  openrgb-fade = pkgs.callPackage ./package.nix { };
  cfg = config.services.openrgb-fade;
in
{
  options.services.openrgb-fade.enable = lib.mkEnableOption "openrgb-fade user service";

  config = lib.mkIf config.services.openrgb-fade.enable {
    systemd.user.services.openrgb-fade = {
      enable = true;
      description = "OpenRGB-Fade user service.";
      serviceConfig = {
        Type = "simple";
        ExecStart = "${openrgb-fade}/bin/openrgb-fade";
      };
      wantedBy = [ "default.target" ];
    };
  };
} 