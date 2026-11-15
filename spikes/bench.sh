#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# spikes/bench.sh - M0.2 UI toolkit benchmark runner (docs/adr/001-ui-toolkit.md).
#
# Builds and runs all four spikes at --scale 1 and --scale 4 (600 frames
# each, scene::FRAMES default), writing spikes/results/<toolkit>-x<scale>.json
# and spikes/results/environment.txt. Run inside `nix develop .#spikes`
# (needs cargo, vulkaninfo, $SPIKE_FONT).
#
# The Nix sandbox has no GPU/display: `nix flake check`'s build-only
# checks only prove each spike compiles. This script, run on a real
# machine, is the only source of the frame-time numbers the ADR's
# Decision section cites.

set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$script_dir"

results_dir="$script_dir/results"
mkdir -p "$results_dir"

if [ -z "${WAYLAND_DISPLAY:-}" ] && [ -z "${DISPLAY:-}" ]; then
  echo "bench.sh: neither \$WAYLAND_DISPLAY nor \$DISPLAY is set; refusing to run (the Nix sandbox has no GPU/display, so this must run on a real machine)." >&2
  exit 2
fi

display_server="x11"
if [ -n "${WAYLAND_DISPLAY:-}" ]; then
  display_server="wayland"
fi

if [ "$display_server" = "x11" ]; then
  echo "bench.sh: warning: running under X11. gpui's present mode is hard-coded to Fifo (vsync) on X11 (see docs/adr/001-ui-toolkit.md), so its frame_ms will be capped at the refresh rate rather than reflecting raw throughput." >&2
fi

# Fast toolkits first, gpui last: gpui's immediate-mode paint_quad path
# is far slower than the others at this primitive count (see the step 6
# commit message), so ordering this way surfaces the other three
# toolkits' results quickly even if gpui's run takes a long time.
toolkits=(egui iced slint gpui)
scales=(1 4)

declare -A dirs=(
  [egui]="$script_dir/spike-egui"
  [iced]="$script_dir/spike-iced"
  [slint]="$script_dir/spike-slint"
  [gpui]="$script_dir/spike-gpui"
)

echo "bench.sh: building release binaries..."
for toolkit in "${toolkits[@]}"; do
  echo "  building $toolkit..."
  if ! (cd "${dirs[$toolkit]}" && cargo build --release --quiet) 2>"$results_dir/$toolkit-build.log"; then
    echo "bench.sh: $toolkit failed to build; see results/$toolkit-build.log" >&2
  fi
done

run_one() {
  local toolkit="$1"
  local scale="$2"
  local bin="${dirs[$toolkit]}/target/release/spike-$toolkit"
  local out="$results_dir/$toolkit-x$scale.json"

  if [ ! -x "$bin" ]; then
    printf '{"toolkit":"%s","scale":%s,"failed":true,"error":"binary not built; see results/%s-build.log"}\n' \
      "$toolkit" "$scale" "$toolkit" >"$out"
    echo "bench.sh: $toolkit x$scale: binary missing, recorded as failed" >&2
    return
  fi

  echo "bench.sh: running $toolkit --scale $scale (this can take a while; gpui especially) ..."

  local stderr_file
  stderr_file="$(mktemp)"
  local json
  local run_rc=0
  if [ "$toolkit" = "iced" ]; then
    json="$(ICED_PRESENT_MODE=mailbox "$bin" --bench --scale "$scale" 2>"$stderr_file")" || run_rc=$?
  else
    json="$("$bin" --bench --scale "$scale" 2>"$stderr_file")" || run_rc=$?
  fi

  if [ "$run_rc" -eq 0 ] && [ -n "$json" ]; then
    printf '%s\n' "$json" >"$out"
  else
    local panic_text
    panic_text="$(tail -c 4000 "$stderr_file" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr '\n' ' ')"
    # Always record the exit code, even when stderr was empty (e.g. a
    # SIGKILL gives the process no chance to print anything) - an empty
    # error string with no exit code is useless for triage later.
    printf '{"toolkit":"%s","scale":%s,"failed":true,"error":"exit code %s: %s"}\n' \
      "$toolkit" "$scale" "$run_rc" "$panic_text" >"$out"
    echo "bench.sh: $toolkit x$scale: run failed (exit $run_rc), recorded error text in $out" >&2
  fi
  rm -f "$stderr_file"
}

for toolkit in "${toolkits[@]}"; do
  for scale in "${scales[@]}"; do
    run_one "$toolkit" "$scale"
  done
done

# Tries, in order, the compositor/display tools that can report the focused
# monitor's refresh rate: hyprctl (Hyprland), wlr-randr (other wlroots
# compositors), xrandr (X11). Only falls back to the "not obtainable"
# placeholder when none of them is on PATH or none yields a usable value.
probe_refresh_rate() {
  if command -v hyprctl >/dev/null 2>&1; then
    local hz
    hz="$(hyprctl monitors 2>/dev/null \
      | grep -m1 -E '@[0-9]+(\.[0-9]+)?[[:space:]]+at[[:space:]]' \
      | sed -E 's/.*@([0-9]+(\.[0-9]+)?)[[:space:]]+at[[:space:]].*/\1/')"
    if [ -n "$hz" ]; then
      echo "${hz}Hz (hyprctl monitors, focused monitor)"
      return
    fi
  fi
  if command -v wlr-randr >/dev/null 2>&1; then
    local hz
    hz="$(wlr-randr 2>/dev/null | grep -m1 -oE '[0-9]+\.[0-9]+ Hz \(current\)' | grep -oE '^[0-9]+\.[0-9]+')"
    if [ -n "$hz" ]; then
      echo "${hz}Hz (wlr-randr, current mode)"
      return
    fi
  fi
  if command -v xrandr >/dev/null 2>&1; then
    local hz
    hz="$(xrandr 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\*' | head -n1 | tr -d '*')"
    if [ -n "$hz" ]; then
      echo "${hz}Hz (xrandr, current mode)"
      return
    fi
  fi
  echo "not obtainable (no hyprctl/wlr-randr/xrandr on PATH, or none reported a current mode)"
}

echo "bench.sh: writing results/environment.txt ..."
{
  echo "uname -a: $(uname -a)"
  if command -v vulkaninfo >/dev/null 2>&1; then
    adapter_line="$(vulkaninfo --summary 2>/dev/null | grep -m1 -i 'deviceName' || true)"
    echo "vulkaninfo adapter: ${adapter_line:-not obtainable}"
  else
    echo "vulkaninfo adapter: vulkaninfo not found on PATH"
  fi
  echo "XDG_SESSION_TYPE: ${XDG_SESSION_TYPE:-not set}"
  echo "display_server (WAYLAND_DISPLAY/DISPLAY check): $display_server"
  echo "monitor refresh rate: $(probe_refresh_rate)"
  echo "spikes git commit: $(git -C "$script_dir" rev-parse HEAD 2>/dev/null || echo unknown)"
} >"$results_dir/environment.txt"

echo "bench.sh: done. Results in $results_dir/"
