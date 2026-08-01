{
  config,
  lib,
  pkgs,
  ...
}:
let 
  openrgb-fade = pkgs.callPackage ./package.nix { };
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