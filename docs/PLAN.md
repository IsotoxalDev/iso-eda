# iso-eda — build plan

License GPL-3.0-or-later. Linux first. Full plan with research and sources:
https://claude.ai/artifact/TdkYbXExt45AJucQKWy1vY (this file is the working summary).

## Goal
A Rust desktop cockpit that replaces the Virtuoso front end for analog/custom IC
design. It drives the open-source engines in IIC-OSIC-TOOLS first; native Rust
engines replace them only where measured to be worth it. Full Cadence parity is
not the target; parity of *workflow* is: library manager, schematic editor,
simulation environment, waveform viewer, layout editor, DRC/LVS/PEX with cross-probing.

## Decisions
| Topic | Choice |
| --- | --- |
| Strategy | Wrap engines as subprocesses first, go native later |
| UI | Slint shell (GPL-3.0-only option, pinned) + our own wgpu canvas (`canvas-wgpu`, from the M0.2 spike). Decided by ADR-001 on measurements: no toolkit's native primitives were viable (gpui 1322 ms, egui 205 ms, iced 98 ms p95 vs 16.7 ms bar; slint+canvas 5.1 ms). Fallback shell is egui (`egui_wgpu::Callback`); gpui lacks a Linux texture-import hook. Binaries are GPL-3.0-only while Slint is linked |
| Schematic format | xschem `.sch` / `.sym` (native, lossless round-trip) |
| Layout format | GDSII (OASIS read) |
| Build | Nix flake: devShell, package (crane), checks |
| Scripting | All actions are `eda-cmd` commands from day one; Python (PyO3) + Tcl in Phase 4 |
| License | GPL-3.0-or-later; ports only from compatible licenses, logged in THIRD-PARTY.md |

## Workspace
```text
iso-eda/
  flake.nix  rust-toolchain.toml  CLAUDE.md  LICENSE
  .claude/agents/  .claude/commands/
  docs/  PLAN.md MILESTONES.md MODEL-ROUTING.md adr/ formats/ manual-tests/
  crates/
    eda-geom     points, rects, transforms, integer database units
    eda-db       Library/Cell/View model, schematic + layout data
    eda-cmd      command registry, undo/redo
    eda-xschem   .sch/.sym reader + writer, attribute/format evaluation
    eda-pdk      PDK discovery ($PDK_ROOT, $PDK), models, layer maps
    eda-netlist  connectivity, SPICE netlister
    eda-sim      Simulator trait, ngspice backend, job runner
    eda-wave     raw/VCD/FST readers, measurements
    eda-gds      GDSII/OASIS/LEF/DEF (Phase 3)
    eda-verify   DRC/LVS/PEX runners + parsers (Phase 3)
    eda-render   toolkit-independent scene -> draw list, culling, LOD, hit test
    eda-script   Python/Tcl bindings (Phase 4)
    eda-ui       Slint shell (.slint panels, themes, input -> eda-cmd) + wgpu canvas (draw-list sink)
    eda-app      GUI binary (`iso-eda`)
    eda-cli      headless binary
  tests/corpus/  fetched by Nix at pinned revisions, never committed
```

## Compatibility targets
- Tools: xschem (files), ngspice then Xyce/VACASK, Magic, KLayout, Netgen,
  klayout-pex, later Yosys/OpenROAD/LibreLane.
- PDKs: sky130A, gf180mcuD (first-class); ihp-sg13g2 (needs OSDI models, test early);
  PTM via a generic PDK template (simulation only); Cadence gpdk180/090 user-supplied,
  simulation only via a Spectre-to-ngspice model converter (Phase 6), never redistributed.
- Known trap: some PDK symbols embed Tcl expressions evaluated at netlist time.
  M1.2 measures how many; `eda-xschem` gets a small evaluator sized to that.

## Phases and exit tests
0. Foundations — flake check green, ADR-001, shell launches.
1. Schematic + simulate — sky130 inverter and 5T OTA drawn from scratch simulate;
   netlists equivalent to xschem's (Netgen) across the corpus.
2. Simulation environment — corners + Monte Carlo on the OTA from a saved setup.
3. Layout + verification — inverter DRC/LVS clean from inside the app; post-layout sim.
4. Scripting — Phase 1 and 3 exit tests run from a Python script, no GUI.
5. Digital cockpit — small RTL block to GDS with waveforms.
6. Native engines — each must beat the wrapped tool on a benchmark.

## Testing layers
Unit (nextest) · property (proptest) · corpus parse · netlist equivalence
(xschem + netgen) · simulation regression (ngspice) · snapshots (insta, PNG diff)
· fuzz (cargo-fuzz) · performance (criterion) · manual checklists in docs/manual-tests/.
Oracle environment: a pinned IIC-OSIC-TOOLS container tag.

## UI rules
One window, dockable panels, tabs per view. Command palette for everything.
Property editing in a side panel, never modal. Themes from one token file.
Bindkey presets: Virtuoso-style and xschem-style.
