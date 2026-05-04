{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    crane,
    self,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
      };
      lib = pkgs.lib;
      gitTag = self.shortRev or "dev";

      craneLib = (crane.mkLib pkgs).overrideToolchain (
        p:
          p.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
          }
      );
      src = craneLib.cleanCargoSource ./.;

      fileSetForCrate = crate:
        lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            (craneLib.fileset.commonCargoSources crate)
          ];
        };

      commonRustArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonRustArgs;

      individualCrateArgs =
        commonRustArgs
        // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;
          # we disable tests since we'll run them all via cargo-nextest
          doCheck = false;
        };

      apiPackages = import ./apps/api/nix/packages.nix {
        inherit pkgs craneLib individualCrateArgs fileSetForCrate gitTag;
      };

      inherit (apiPackages) api apiDockerImage;

      embedImageServiceFileset = lib.fileset.unions [
        ./pnpm-lock.yaml
        ./pnpm-workspace.yaml
        ./package.json
        ./apps/embed-image-service/package.json
        ./apps/embed-image-service/tsconfig.json
        ./apps/embed-image-service/src
        ./packages/embed-image/package.json
        ./packages/embed-image/tsconfig.json
        ./packages/embed-image/vite.config.ts
        ./packages/embed-image/src
        ./packages/config/tsconfig
      ];

      embedImageServiceSrc = lib.fileset.toSource {
        root = ./.;
        fileset = embedImageServiceFileset;
      };

      embedImageServicePnpmDeps = pkgs.fetchPnpmDeps {
        pname = "embed-image-service";
        version = gitTag; #"0.1.0";
        src = embedImageServiceSrc;
        fetcherVersion = 3;
        hash = "sha256-llxysmPg44mu87bM4/UUHlIHbayPXpIBm4DDNWWIDiQ=";
      };

      embedImageServicePackages = import ./apps/embed-image-service/nix/packages.nix {
        inherit pkgs embedImageServiceSrc embedImageServicePnpmDeps gitTag;
      };

      inherit (embedImageServicePackages) embedImageService embedImageServiceDockerImage;
    in {
      checks = {
        inherit api embedImageService;

        workspace-clippy = craneLib.cargoClippy (
          commonRustArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          }
        );
        workspace-fmt = craneLib.cargoFmt {
          inherit src;
        };

        workspace-nextest = craneLib.cargoNextest (
          commonRustArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          }
        );
      };
      devShells.default = with pkgs;
        craneLib.devShell {
          checks = self.checks.${system};

          packages = [
            nodejs_25
            pnpm_10
            pkg-config
            openssl
            just
            mprocs
          ];

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig";
            export PATH="$PWD/node_modules/.bin/:$PATH"
          '';
        };

      packages = {
        inherit api apiDockerImage embedImageService embedImageServiceDockerImage;
      };
    });
}
