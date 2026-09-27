{
  pkgs,
  craneLib,
  individualCrateArgs,
  fileSetForCrate,
  gitTag,
}: let
  gifService = craneLib.buildPackage (
    individualCrateArgs
    // {
      pname = "gif-service";
      cargoExtraArgs = "-p gif-service";
      src = fileSetForCrate ./..;
    }
  );

  gifServiceDockerImage = pkgs.dockerTools.buildLayeredImage {
    name = "abembed-gif-service";
    tag = gitTag;

    contents = [
      gifService
      pkgs.cacert
      pkgs.ffmpeg-headless
    ];

    config = {
      Cmd = ["${gifService}/bin/gif-service"];
      User = "65532:65532";
      ExposedPorts = {
        "3002/tcp" = {};
      };
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  };
in {
  inherit gifService gifServiceDockerImage;
}
