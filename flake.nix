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

        # crane's default cargo-source filter only keeps Rust/Cargo files;
        # `eda-ui` also needs its `.slint` UI sources (compiled by
        # `build.rs`) and its keymap data files to be present in the
        # sandboxed build.
        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*\\.slint$" path != null)
            || (builtins.match ".*/keymap/.*" path != null);
        };

        # `eda-ui`/`eda-app` link Slint (GPL-3.0-only option) + wgpu for the
        # M0.3 shell and canvas. This is the same system-library set ADR-001
        # recorded working for the M0.2 spikes' `devShells.spikes` (deleted
        # in 8e28d49/443dd94 along with `spikes/`), now adopted permanently
        # since the dependency itself moved into the main workspace. No
        # clang/cmake/lld/libclang: those were only needed for gpui's C
        # deps, which this project does not use.
        uiNativeBuildInputs = [ pkgs.pkg-config ];
        uiBuildInputs = with pkgs; [
          fontconfig
          freetype
          libxkbcommon
          wayland
          wayland-protocols
          vulkan-loader
          vulkan-headers
          libGL
          mesa
          libxcb
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          xorg.libxcb
          xorg.xcbutilimage
          xorg.xcbutilkeysyms
          xorg.xcbutilwm
          xorg.xcbutilrenderutil
        ];

        commonArgs = {
          inherit src;
          strictDeps = true;
          # The root Cargo.toml is workspace-only (no [package]), so crane
          # cannot infer a pname/version for it; set them explicitly to
          # silence the "will use a placeholder value" evaluation warnings.
          pname = "iso-eda-workspace";
          version = "0.1.0";

          nativeBuildInputs = uiNativeBuildInputs;
          buildInputs = uiBuildInputs;
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
          ] ++ uiNativeBuildInputs ++ uiBuildInputs;

          # Runtime dlopen targets for Slint's winit backend + wgpu's
          # Vulkan loader (same list ADR-001 recorded for the M0.2 spikes'
          # `devShells.spikes`).
          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.vulkan-loader
              pkgs.libGL
              pkgs.wayland
              pkgs.libxkbcommon
              pkgs.xorg.libxcb
              pkgs.xorg.libX11
            ]}:$LD_LIBRARY_PATH
          '';
        };
      });
}
