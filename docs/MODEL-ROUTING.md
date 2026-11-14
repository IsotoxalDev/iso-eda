# Model routing: Fable vs Sonnet

Principle: Fable decides *what and why*; Sonnet does *the building*. Fable is
slower and costs more, so use it where a wrong decision is expensive to undo.

## Rubric
Score one point for each that applies:
1. Defines data structures or traits other crates will depend on.
2. Requires reverse-engineering behaviour (xschem attribute semantics, raw-file quirks).
3. Correctness is subtle and silent when wrong (connectivity, netlisting, geometry, undo).
4. Performance-critical architecture (render pipeline, spatial index, large GDS).
5. Decision between alternatives with long-term cost (toolkit, file format, API shape).
6. Debugging where the cause is unknown.

0-1 points: **S** (Sonnet). 2+ points: **F** (Fable designs and reviews, Sonnet implements).

## Always Fable
- ADRs and design notes; phase planning; milestone breakdown for the next phase
- Core data model (`eda-db`), command/undo model (`eda-cmd`), render architecture
- xschem format semantics, Tcl-expression evaluation, connectivity and netlisting
- DRC/LVS result mapping and cross-probing design; scripting API design
- Final review of every Tier F diff; root-cause analysis after Sonnet escalates

## Always Sonnet
- Scaffolding, Cargo/Nix/CI wiring, formatting, lint fixes
- Implementing a parser or writer from an agreed format note
- Subprocess runners, log parsing, CLI subcommands, PDK path discovery
- UI panels, widgets, keymaps and theming from an agreed design
- Unit/property/snapshot tests, fuzz targets, benchmarks, docs, manual-test checklists
- Mechanical refactors and dependency bumps

## How it runs in Claude Code
- Agents in `.claude/agents/` pin a model in their frontmatter. Claude Code
  delegates to them by matching the task to each agent's description.
- `/milestone M1.2` reads the Tier and follows the matching workflow.
- Start the main session on Sonnet for Tier S milestones and on Fable for Tier F
  (`/model` switches mid-session).
