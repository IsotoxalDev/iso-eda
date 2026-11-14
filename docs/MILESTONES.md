# Milestones

Tier: **F** = Fable designs + reviews, Sonnet implements. **S** = Sonnet throughout.
Status: todo / doing / done.

## Phase 0 — Foundations

| ID | Task | Tier | Acceptance check | Status |
| --- | --- | --- | --- | --- |
| M0.1 | Workspace skeleton, flake (dev shell, package, checks), CI, empty crates, GPL headers, LICENSE | S | `nix develop` and `nix flake check` pass on a clean clone | done |
| M0.2 | UI spike: throwaway GPUI, Slint and egui binaries drawing 200k rects, 50k segments, 5k labels with pan/zoom | F (analysis + ADR-001), S (the three binaries) | Frame times recorded; ADR-001 states the choice | todo |
| M0.3 | App shell: window, docks, command palette, keymap file, theme tokens, empty canvas | S | Launches with `nix run`; palette runs a dummy command; theme switch works | todo |

## Phase 1 — Schematic + simulate

| ID | Task | Tier | Acceptance check | Status |
| --- | --- | --- | --- | --- |
| M1.1 | `eda-geom`, `eda-db`, `eda-cmd`: library/cell/view model, schematic objects, command + undo stack | F | Unit + property tests; undo then redo returns identical state | todo |
| M1.2 | `eda-xschem`: read/write `.sch` and `.sym` | F (format note, attribute semantics, Tcl-usage survey), S (parser/writer) | Whole corpus parses; read-write-read lossless; report count of symbols using Tcl expressions | todo |
| M1.3 | `eda-pdk`: discover sky130A, gf180mcuD, ihp-sg13g2 from `$PDK_ROOT` | S | `iso-eda pdk list` / `pdk info sky130A` correct inside the IIC-OSIC-TOOLS container | todo |
| M1.4 | `eda-render` + read-only schematic view | F (scene/draw-list/culling design), S (implementation) | Headless PNG snapshots of 20 corpus schematics; manual comparison with xschem | todo |
| M1.5 | Editing: select, move, rotate, mirror, wire with rubber-banding, place, property panel, copy/paste | S (F only for wire-drag/connectivity-preserving move) | Each operation is a command with a unit test; manual checklist passes | todo |
| M1.6 | `eda-netlist`: connectivity + SPICE netlisting from `format` attributes | F | Netgen reports our netlist equivalent to xschem's for every corpus schematic | todo |
| M1.7 | `eda-sim`: ngspice runner, streamed log, errors mapped to instances | S | Inverter DC/AC/tran run from CLI; broken netlist highlights the right instance | todo |
| M1.8 | `eda-wave` + waveform panel: raw reader, plots, cursors, measurements | S | Values match ngspice `print` within tolerance; manual checklist passes | todo |
| M1.9 | Hierarchy: descend/ascend, symbol from schematic, subcircuit netlisting | F | Two-level design netlists equivalent to xschem's | todo |
| M1.10 | Phase 1 release: examples, first-run flow, user docs; draft Phase 2 milestone table | S (docs, examples), F (Phase 2 table) | Phase 1 exit test passes on a fresh machine | todo |

## Later phases (broken down when reached)

| Phase | Mostly F | Mostly S |
| --- | --- | --- |
| 2 Simulation environment | Testbench/corner/Monte Carlo data model, measurement expression language, op-point back-annotation | Xyce/VACASK runners, result tables, plots, setup persistence |
| 3 Layout + verification | Layout DB, render LOD for large GDS, editing geometry ops, cross-probe design | GDS/OASIS/LEF I/O, `.lyp`/tech parsing, DRC/LVS/PEX runners and report parsers, marker browser |
| 4 Scripting | Python/Tcl API shape and stability policy | PyO3 bindings, console panel, examples |
| 5 Digital cockpit | Mixed-signal setup model | Flow runners, report viewers, VCD/FST readers |
| 6 Native engines | All algorithm design (DRC, SDL, pcells, Spectre model import) | Rule-deck plumbing, benchmarks, ports to macOS/Windows |
