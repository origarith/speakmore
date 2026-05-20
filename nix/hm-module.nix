# Home-manager module for SpeakMore speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ speakmore.homeManagerModules.default ];
#        services.speakmore.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.speakmore;
in
{
  options.services.speakmore = {
    enable = lib.mkEnableOption "SpeakMore speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "speakmore.packages.\${system}.speakmore";
      description = "The SpeakMore package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.speakmore = {
      Unit = {
        Description = "SpeakMore speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/speakmore";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
