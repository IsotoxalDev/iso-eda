# iso-eda — rules for Claude Code

iso-eda is an open-source (GPL-3.0-or-later) EDA suite in Rust: a modern front end
for analog/custom IC design that drives the engines in IIC-OSIC-TOOLS.
Linux first. Read `docs/PLAN.md` and the milestone row in `docs/MILESTONES.md`
before starting. Work on exactly one milestone per session.

## Build and verify
- Enter the environment with `nix develop`. Never install tools globally.
- A task is done only when `nix flake check` passes. Run it before you report.
- Tests: `cargo nextest run`. Snapshot review (`cargo insta review`) is done by the human.

## Model routing (who does what)
Each milestone in `docs/MILESTONES.md` has a Tier: **F** (Fable) or **S** (Sonnet).
- Tier F: design, semantics, architecture, hard debugging. The `architect` agent
  (Fable) writes the design note or ADR first; the `implementer` agent (Sonnet)
  then builds it; the `reviewer` agent (Fable) reviews the diff before the report.
- Tier S: the `implementer` agent builds directly; `test-engineer` (Sonnet) adds tests.
- Escalate from Sonnet to `architect` and stop coding when any of these is true:
  1. the same test has failed after two distinct fix attempts;
  2. the change needs a new public type or trait in eda-db, eda-cmd or eda-render;
  3. the change crosses more than two crates or reverses the dependency direction;
  4. file-format or netlisting behaviour is ambiguous and the docs do not settle it;
  5. `unsafe`, concurrency primitives, or a new dependency is needed;
  6. a golden/oracle comparison disagrees and the cause is not obvious.
- If a milestone has no Tier, classify it with the rubric in `docs/MODEL-ROUTING.md`,
  write the Tier into the table, and say so in the report.

## Architecture
- Dependency direction: geom -> db -> format/engine crates -> cmd -> render -> ui -> app.
- Only `eda-ui` and `eda-app` may depend on the UI toolkit (GPUI, pinned revision).
- Every user-visible action is a command registered in `eda-cmd`, with undo support.
- Geometry uses integer database units. No floats in `eda-db`.
- External engines are called through traits in `eda-sim` and `eda-verify`,
  as subprocesses first. The UI never blocks on an engine.

## Code
- No `unsafe` without a comment explaining why and a test covering it.
- No `unwrap`/`expect` outside tests. `thiserror` in libraries, `anyhow` in binaries.
- New dependency: state crate, license (must be GPL-3.0-compatible) and reason; wait for approval.
- Parsers must never panic on bad input. Each parser gets a fuzz target.

## Licensing (GPL-3.0-or-later)
- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- Code may be adapted only from sources whose license is compatible with
  GPL-3.0-or-later (GPL-2.0-or-later, GPL-3.0, LGPL, BSD, MIT, Apache-2.0).
  Not GPL-2.0-only. Check the upstream license file first, ask the human before
  porting, keep the original copyright notice, and log it in `docs/THIRD-PARTY.md`.
- Never add proprietary PDK files (e.g. Cadence gpdk) to the repo or test corpus.

## Testing
- Write the tests named in the milestone first, then the code.
- Reference outputs come from the real tools (xschem, ngspice, netgen), never by hand.
  Record tool versions and the IIC-OSIC-TOOLS tag next to golden files.
- Tests that need a PDK or external tool skip cleanly when it is missing.

## Reporting
End each session with: what changed; what was verified and how; what was not
verified; which model tier did which part; and an updated
`docs/manual-tests/<milestone>.md` for the human. Record design decisions as
`docs/adr/NNN-title.md` (template: `docs/adr/000-template.md`).
