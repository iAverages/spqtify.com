{
  pkgs,
  craneLib,
  individualCrateArgs,
  fileSetForCrate,
  gitTag,
}: let
  api = craneLib.buildPackage (
    individualCrateArgs
    // {
      pname = "api";
      cargoExtraArgs = "-p api";
      src = fileSetForCrate ./..;
    }
  );

  apiDockerImage = pkgs.dockerTools.buildLayeredImage {
    name = "spqtify-api";
    tag = gitTag;

    contents = [
      api
      pkgs.cacert
    ];

    config = {
      Cmd = ["${api}/bin/api"];
      ExposedPorts = {
        "3000/tcp" = {};
      };
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  };
in {
  inherit api apiDockerImage;
}
