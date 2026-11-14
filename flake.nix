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

        # --- M0.2 UI-toolkit spikes (docs/adr/001-ui-toolkit.md) ---
        # Throwaway binaries under spikes/, outside the root Cargo
        # workspace (root Cargo.toml excludes it). They pin their own,
        # newer toolchain (gpui/Slint need it) and are proven only to
        # *compile* in the Nix sandbox: the sandbox has no GPU/display, so
        # frame times come from spikes/bench.sh on a real machine, never
        # from `nix flake check`.
        spikesToolchain = pkgs.rust-bin.fromRustupToolchainFile ./spikes/rust-toolchain.toml;
        spikesCraneLib = (crane.mkLib pkgs).overrideToolchain (_: spikesToolchain);
        spikesSrc = spikesCraneLib.cleanCargoSource ./spikes;

        spikeNativeBuildInputs = [ pkgs.pkg-config pkgs.clang pkgs.lld pkgs.cmake ];
        # Rachana's verified-to-build-gpui-under-Nix list (~/Dev/Rachana
        # flake.nix); adopted rather than guessing a smaller one.
        spikeBuildInputs = with pkgs; [
          openssl
          fontconfig
          freetype
          libxkbcommon
          wayland
          wayland-protocols
          vulkan-loader
          vulkan-headers
          libGL
          mesa
          alsa-lib
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
        spikeEnv = {
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          VULKAN_SDK = "${pkgs.vulkan-headers}";
        };

        # A crate living at spikes/<name> with its own Cargo.toml/Cargo.lock
        # and an empty [workspace] table (so it is not swept into the root
        # workspace). crane's documented pattern for a package in a
        # subdirectory with sibling path deps: unpack the whole spikes/
        # tree, then cd into the one crate before the build proper runs.
        spikeCrateArgs = name: extra: ({
          src = spikesSrc;
          cargoToml = ./spikes/${name}/Cargo.toml;
          cargoLock = ./spikes/${name}/Cargo.lock;
          strictDeps = true;
          pname = name;
          version = "0.1.0";
          postUnpack = ''
            cd $sourceRoot/${name}
            sourceRoot="."
          '';
        } // extra);

        spikeSceneArgs = spikeCrateArgs "scene" { };
        spikeSceneArtifacts = spikesCraneLib.buildDepsOnly spikeSceneArgs;

        # Build-only checks: the sandbox has no GPU/display, so these only
        # prove compilation (docs/adr/001-ui-toolkit.md "Nix sandbox").
        spikeEguiArgs = spikeCrateArgs "spike-egui" {
          nativeBuildInputs = spikeNativeBuildInputs;
          buildInputs = spikeBuildInputs;
        } // spikeEnv;
        spikeEguiArtifacts = spikesCraneLib.buildDepsOnly spikeEguiArgs;
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

          spike-scene-test = spikesCraneLib.cargoNextest (spikeSceneArgs // {
            cargoArtifacts = spikeSceneArtifacts;
          });

          spike-egui-build = spikesCraneLib.buildPackage (spikeEguiArgs // {
            cargoArtifacts = spikeEguiArtifacts;
            doCheck = false;
          });
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.cargo-nextest
          ];
        };

        devShells.spikes = pkgs.mkShell {
          nativeBuildInputs = spikeNativeBuildInputs;
          buildInputs = [ spikesToolchain pkgs.cargo-nextest pkgs.vulkan-tools ] ++ spikeBuildInputs;
          inherit (spikeEnv) LIBCLANG_PATH VULKAN_SDK;
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
              pkgs.vulkan-loader
              pkgs.libGL
              pkgs.wayland
              pkgs.libxkbcommon
              pkgs.libxcb
              pkgs.xorg.libX11
            ]}:$LD_LIBRARY_PATH"
            export SPIKE_FONT="${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf"
          '';
        };
      });
}
