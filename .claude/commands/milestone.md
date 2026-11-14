---
description: Run one milestone from docs/MILESTONES.md with the right model tier
argument-hint: <milestone id, e.g. M1.2>
---
Milestone $ARGUMENTS.

1. Read CLAUDE.md, docs/PLAN.md, and the row for $ARGUMENTS in docs/MILESTONES.md.
   If it has no Tier, classify it with docs/MODEL-ROUTING.md and write the Tier in.
2. Present a plan: files to add, tests to write, which agent does each part, and
   anything in the acceptance check you cannot verify yourself. Wait for my OK.
3. Tier F parts: `architect` writes the design note first; show it to me.
   Then `implementer` builds; `test-engineer` covers the test list.
   Tier S parts: `implementer` and `test-engineer` directly.
4. Run `nix flake check`. For Tier F, run `reviewer` on the diff and fix findings.
5. Set Status in docs/MILESTONES.md and finish with the report format in CLAUDE.md.
