{
  description = "iso-eda: a GPL-3.0-or-later EDA suite for analog/custom IC design";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
          # The root Cargo.toml is workspace-only (no [package]), so crane
          # cannot infer a pname/version for it; set them explicitly to
          # silence the "will use a placeholder value" evaluation warnings.
          pname = "iso-eda-workspace";
          version = "0.1.0";
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        workspace = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        isoEdaApp = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "iso-eda";
          cargoExtraArgs = "--package eda-app --bin iso-eda";
        });

        isoEdaCli = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "iso-eda-cli";
          cargoExtraArgs = "--package eda-cli --bin iso-eda-cli";
        });

      in
      {
        packages = {
          default = isoEdaApp;
          iso-eda = isoEdaApp;
          iso-eda-cli = isoEdaCli;
        };

        checks = {
          workspace-build = workspace;

          workspace-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace -- -D warnings";
          });

          workspace-fmt = craneLib.cargoFmt (commonArgs // { });

          workspace-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });

          spdx-headers = pkgs.runCommand "spdx-headers-check"
            {
              nativeBuildInputs = [ pkgs.bash pkgs.gnugrep pkgs.findutils ];
            }
            ''
              bash ${./scripts/check-spdx.sh} ${self}
              touch $out
            '';
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.cargo-nextest
          ];
        };
      });
}
