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

        # eda-app dlopens Vulkan/Wayland/X11/fontconfig/freetype at runtime
        # (Slint's winit backend, wgpu's loader, Slint's own font-matching
        # via fontconfig) in addition to needing them at link time, so the
        # packaged binary needs its runtime linker search path wrapped —
        # `buildInputs` alone only covers the build. Without this, `nix run`
        # fails with e.g. "error while loading shared libraries:
        # libfontconfig.so.1: cannot open shared object file".
        isoEdaApp = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "iso-eda";
          cargoExtraArgs = "--package eda-app --bin iso-eda";
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          postInstall = ''
            wrapProgram $out/bin/iso-eda \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath uiBuildInputs}
          '';
        });

        isoEdaCli = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "iso-eda-cli";
          cargoExtraArgs = "--package eda-cli --bin iso-eda-cli";
        });

        # crates/eda-ui/fuzz (M0.3, CLAUDE.md: "Each parser gets a fuzz
        # target"): a standard cargo-fuzz layout with its own `[workspace]`
        # and Cargo.lock, so it needs its own vendor dir rather than reusing
        # `cargoArtifacts` above. This check only *compiles* the fuzz
        # target (`craneLib.cargoBuild`, the same "build-only, no sandbox
        # execution" shape ADR-001 used for the M0.2 spike checks) -- it
        # does not run `cargo fuzz build`/`cargo fuzz run`. `cargo-fuzz`
        # itself is not packaged into this devShell/sandbox (it typically
        # wants LLVM sanitizer-coverage flags that this workspace's pinned
        # stable toolchain does not add by default), and an actual fuzzing
        # campaign is a developer running it locally, not part of
        # `nix flake check`. A plain `cargo build --manifest-path
        # crates/eda-ui/fuzz/Cargo.toml` was confirmed to compile the fuzz
        # target on the pinned stable 1.98.1 toolchain before wiring this
        # check in.
        fuzzCargoVendorDir = craneLib.vendorCargoDeps {
          cargoLock = ./crates/eda-ui/fuzz/Cargo.lock;
        };

        fuzzCommonArgs = {
          inherit src;
          strictDeps = true;
          pname = "eda-ui-fuzz";
          version = "0.0.0";
          cargoVendorDir = fuzzCargoVendorDir;
          cargoExtraArgs = "--manifest-path crates/eda-ui/fuzz/Cargo.toml";
          nativeBuildInputs = uiNativeBuildInputs;
          buildInputs = uiBuildInputs;
        };

        fuzzCargoArtifacts = craneLib.buildDepsOnly fuzzCommonArgs;

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

          eda-ui-fuzz-build = craneLib.cargoBuild (fuzzCommonArgs // {
            cargoArtifacts = fuzzCargoArtifacts;
          });

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

          # Runtime dlopen targets for Slint's winit backend, wgpu's Vulkan
          # loader, and Slint's own fontconfig-based font matching. This
          # used to be a hand-picked subset of `uiBuildInputs` (the list
          # ADR-001 recorded for the M0.2 spikes' `devShells.spikes`, which
          # never linked Slint's font stack and so never needed
          # fontconfig/freetype at runtime); M0.3 added that need and the
          # subset went stale, breaking `cargo build`-then-run inside this
          # shell with "libfontconfig.so.1: cannot open shared object
          # file". Using the same full list this crate builds against
          # keeps the two from drifting apart again.
          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath uiBuildInputs}:$LD_LIBRARY_PATH
          '';
        };
      });
}
