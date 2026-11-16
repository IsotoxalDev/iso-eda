# ADR-002: Command and undo model for `eda-cmd`

- Status: accepted (2026-11-17, owner approved the design and the
  `thiserror` dependency sign-off below)
- Date: 2026-11-16
- Milestone: M0.3 — App shell (the `eda-cmd` part; Tier F because
  `docs/MODEL-ROUTING.md` lists the command/undo model under "Always Fable"
  and CLAUDE.md escalation rule 2 covers new public types in `eda-cmd`)

## Context

CLAUDE.md: "Every user-visible action is a command registered in `eda-cmd`,
with undo support." ADR-001 adds: all behaviour lives in Rust as `eda-cmd`
commands; `.slint` holds only layout and styling; `eda-ui` maps Slint events
to commands. M0.3's acceptance check is "palette runs a dummy command; theme
switch works", so the palette must be able to list commands, run one by id,
and undo it. `crates/eda-cmd` is an empty skeleton today.

M1.1 ("`eda-geom`, `eda-db`, `eda-cmd`: ... command + undo stack; undo then
redo returns identical state") will put real schematic edits on this shape.
The one subtle property it needs is that **redo replays the recorded change
exactly** — including any object ids allocated the first time — otherwise a
later edit on the redo side that refers to those ids silently targets the
wrong object. The shape chosen here must make that the natural way to write
a command, without inventing a data model now (`eda-db` does not exist yet).

Constraints: no `unwrap`/`expect` outside tests; `thiserror` for library
errors; integer-only rules do not apply (no geometry here); no new
dependency without sign-off; `eda-cmd` may depend on `eda-db` later (direction
`geom -> db -> ... -> cmd -> render -> ui -> app`) but never on the toolkit.
The host is single-threaded (Slint event loop); engines run as subprocesses
behind traits and post results back to the UI thread.

## Options considered (brief)

- **One trait with `execute`/`undo` and redo = re-execute.** Simplest to
  write for M0.3's toggle, but it is exactly the trap described above: an
  M1.1 `AddInstance` whose `execute` allocates a fresh id would corrupt the
  history on redo unless every author remembers to cache the id. A default
  `redo` that silently does the wrong thing fails rubric point 3 ("subtle and
  silent when wrong"). Rejected.
- **Commands as closures (`Box<dyn Fn(&mut Ctx)>`) with inverse closures.**
  Minimal code, but edits carry no label, cannot be inspected or merged
  later, and scripting (Phase 4) cannot enumerate or name them. Rejected.
- **Inverse-patch model (command produces a DB diff; undo applies the
  inverse).** The right end state for `eda-db`, but it requires the data
  model. The `Edit` trait below is deliberately shaped so an M1.1 `Patch`
  type can implement it once; nothing here needs rewriting then.
- **Generic context (`Command<Cx>`) or type-erased state map.** Lets
  `eda-cmd` stay ignorant of `eda-db`, at the cost of generic registries and
  hosts in every UI call site. The dependency direction already allows
  `eda-cmd -> eda-db`, so a concrete context struct that grows a field in
  M1.1 is simpler. Rejected.

## Decision

Split *the request* from *the recorded change*:

- `Command` is a stateless verb (`run(&self, cx) -> Outcome`). The registry
  owns one instance per id for the palette; commands with arguments are plain
  values constructed by `eda-ui` and run through the same host.
- `Edit` is the recorded, undoable change a command returns. It has separate
  `undo` and `redo` methods and **no default for `redo`**: each edit states
  how it replays its captured change. This is the one reason that mattered:
  it makes "undo then redo returns identical state" the path of least
  resistance for M1.1.
- `CommandContext` is a concrete struct owned by `CommandHost`. For M0.3 it
  holds a `Theme` and a status message — real application state that will
  survive `eda-db`'s arrival, not a stand-in data model. M1.1 adds a design
  field; existing commands ignore it.
- `UndoStack` is a bounded linear history: a new edit clears the redo side
  (standard editor behaviour, confirmed); a non-undoable command does not.
- Undo and redo are themselves commands (`edit.undo`, `edit.redo`) so the
  keymap file and the palette treat them like everything else.
- A failed command is never recorded and never touches the redo side.

## Consequences

Easier: `eda-ui` has one object to call (`CommandHost`) and one invariant
to trust (every state change the user can undo went through `run`). M1.1
adds a field to `CommandContext`, implements `Edit` for its patch type, and
adds commands; the registry, stack and host do not change. Scripting later
enumerates `CommandRegistry` and calls `run` by id.

Harder: every undoable command must be written as "compute the change,
return an `Edit`" rather than mutating freely; composite operations need a
`CompoundEdit` (not in M0.3, see "Not solved"). Two traits instead of one.

Reversal: all of this is inside one crate with two trivial commands; if M1.1
finds the split wrong, replacing it costs rewriting four small modules and
the `eda-ui` palette glue, nothing else.

## Public types and traits (signatures only; the implementer builds from this)

Crate layout: `crates/eda-cmd/src/{lib.rs, id.rs, error.rs, context.rs,
command.rs, registry.rs, undo.rs, host.rs, builtins.rs}`. `lib.rs` re-exports
everything below at the crate root. Every file starts with the SPDX line.

```rust
// id.rs
/// Stable, user-visible command identifier: `<area>.<verb_noun>`, ASCII
/// lower-case snake case, e.g. `app.hello`, `view.toggle_theme`,
/// `edit.undo`. Keymap files and the palette refer to commands by this.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(Cow<'static, str>);
impl CommandId {
    pub const fn new(id: &'static str) -> Self;
    pub fn as_str(&self) -> &str;
}
impl From<String> for CommandId;           // for scripting / config later
impl fmt::Display for CommandId;           // prints the bare id

// error.rs  (thiserror)
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("command `{0}` is already registered")]
    DuplicateId(CommandId),
    #[error("unknown command `{0}`")]
    UnknownCommand(CommandId),
    #[error("command `{id}` failed: {source}")]
    Failed { id: CommandId, #[source] source: BoxError },
    #[error("undo of `{label}` failed: {source}")]
    UndoFailed { label: String, #[source] source: BoxError },
    #[error("redo of `{label}` failed: {source}")]
    RedoFailed { label: String, #[source] source: BoxError },
}
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
impl CommandError {
    /// Convenience for command bodies: wraps any error as `Failed`.
    pub fn failed(id: &CommandId, source: impl std::error::Error + Send + Sync + 'static) -> Self;
}

// context.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme { #[default] Dark, Light }
impl Theme {
    pub fn toggled(self) -> Theme;
    /// "dark" | "light" — the key `eda-ui` uses to pick a token set.
    pub fn as_str(self) -> &'static str;
}

/// Everything a command may read or mutate. Owned by `CommandHost`.
/// M1.1 adds the design (`eda_db`) here; nothing else about this type changes.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CommandContext {
    pub theme: Theme,
    /// Last message a command asked the UI to show (status line). Not undoable.
    pub status_message: Option<String>,
}

// command.rs
/// What a command's run produced.
pub enum Outcome {
    /// State was not changed in any undoable way (or not at all).
    Done,
    /// An undoable change was applied; the host records it.
    Edit(Box<dyn Edit>),
    /// Ask the host to pop the undo / redo stack (returned only by the
    /// `edit.undo` / `edit.redo` builtins).
    Undo,
    Redo,
}

/// A user-invocable verb. Stateless: all state is in `cx`; all change is
/// in the returned `Edit`. Must be atomic: on `Err`, `cx` is unchanged.
pub trait Command {
    fn id(&self) -> CommandId;
    /// Palette text, e.g. "Toggle theme".
    fn title(&self) -> &str;
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}

/// A recorded change. `redo` must reproduce exactly the state `run` left
/// behind, using data captured at run time (ids, old/new values); it must
/// not recompute, re-allocate, or re-read the selection.
pub trait Edit {
    /// Shown as "Undo <label>" / "Redo <label>".
    fn label(&self) -> &str;
    fn undo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
    fn redo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
}

// registry.rs
/// Commands the palette can list and run. Ordered by id (BTreeMap) so
/// enumeration is deterministic for the palette and for snapshot tests.
#[derive(Default)]
pub struct CommandRegistry { /* BTreeMap<CommandId, Box<dyn Command>> */ }
impl CommandRegistry {
    pub fn new() -> Self;
    /// All builtins (`app.hello`, `edit.redo`, `edit.undo`, `view.toggle_theme`).
    pub fn with_builtins() -> Self;
    /// `Err(DuplicateId)` leaves the registry unchanged.
    pub fn register(&mut self, cmd: Box<dyn Command>) -> Result<(), CommandError>;
    pub fn get(&self, id: &CommandId) -> Option<&dyn Command>;
    /// Ascending by id.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Command> + '_;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

// undo.rs
pub const DEFAULT_UNDO_LIMIT: usize = 1_000;
/// Linear history: `undo` side (oldest evicted past `limit`) + `redo` side.
pub struct UndoStack { /* VecDeque<Box<dyn Edit>>, Vec<Box<dyn Edit>>, NonZeroUsize */ }
impl Default for UndoStack;                       // DEFAULT_UNDO_LIMIT
impl UndoStack {
    pub fn with_limit(limit: NonZeroUsize) -> Self;
    /// Records an applied edit: clears the redo side, evicts the oldest
    /// undo entry when the undo side would exceed `limit`.
    pub fn push(&mut self, edit: Box<dyn Edit>);
    /// `Ok(false)` when there is nothing to undo. On `Err`, the edit stays
    /// on the undo side (stack unchanged).
    pub fn undo(&mut self, cx: &mut CommandContext) -> Result<bool, CommandError>;
    /// `Ok(false)` when there is nothing to redo. On `Err`, the edit stays
    /// on the redo side.
    pub fn redo(&mut self, cx: &mut CommandContext) -> Result<bool, CommandError>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn undo_label(&self) -> Option<&str>;
    pub fn redo_label(&self) -> Option<&str>;
    pub fn undo_len(&self) -> usize;
    pub fn redo_len(&self) -> usize;
    pub fn limit(&self) -> NonZeroUsize;
    pub fn clear(&mut self);
}

// host.rs
/// The single object `eda-ui` talks to. Owns registry, stack and context
/// so the invariant "every undoable change went through `run`" is local.
pub struct CommandHost { /* CommandRegistry, UndoStack, CommandContext */ }
impl CommandHost {
    pub fn new(registry: CommandRegistry, cx: CommandContext) -> Self;  // default stack
    pub fn with_undo_stack(registry: CommandRegistry, cx: CommandContext, undo: UndoStack) -> Self;
    /// Builtin registry, default context — what `eda-app` constructs.
    pub fn with_builtins() -> Self;
    pub fn registry(&self) -> &CommandRegistry;
    pub fn context(&self) -> &CommandContext;
    pub fn undo_stack(&self) -> &UndoStack;
    /// Look up by id and run (`UnknownCommand` if absent). Applies the
    /// `Outcome`: `Edit` -> push; `Undo`/`Redo` -> pop the stack; `Done` -> nothing.
    pub fn run(&mut self, id: &CommandId) -> Result<(), CommandError>;
    /// Same, for a command value the caller constructed (commands with
    /// arguments, from M1.5 on). Not required to be registered.
    pub fn run_command(&mut self, cmd: &dyn Command) -> Result<(), CommandError>;
    pub fn undo(&mut self) -> Result<bool, CommandError>;
    pub fn redo(&mut self) -> Result<bool, CommandError>;
}

// builtins.rs
pub const HELLO_MESSAGE: &str = "Hello from eda-cmd";
pub struct HelloCommand;        // id "app.hello",          title "Hello"
pub struct ToggleThemeCommand;  // id "view.toggle_theme",  title "Toggle theme"
pub struct UndoCommand;         // id "edit.undo",          title "Undo"
pub struct RedoCommand;         // id "edit.redo",          title "Redo"
/// The edit `ToggleThemeCommand` returns. Captures both ends so redo sets
/// `to` rather than toggling again.
pub struct ThemeEdit { from: Theme, to: Theme }   // label "Toggle theme"
impl ThemeEdit { pub fn new(from: Theme, to: Theme) -> Self; }
pub fn builtin_commands() -> Vec<Box<dyn Command>>;
```

Behaviour of the builtins:

- `HelloCommand::run`: `cx.status_message = Some(HELLO_MESSAGE.to_owned())`;
  returns `Outcome::Done`.
- `ToggleThemeCommand::run`: `from = cx.theme; to = from.toggled(); cx.theme = to`;
  returns `Outcome::Edit(Box::new(ThemeEdit::new(from, to)))`.
- `ThemeEdit::undo`: `cx.theme = self.from`. `redo`: `cx.theme = self.to`.
- `UndoCommand::run` / `RedoCommand::run`: touch nothing; return
  `Outcome::Undo` / `Outcome::Redo`.

No `Send`/`Sync` bounds on `Command` or `Edit`: the host lives on the UI
thread. Background work (M1.7 simulation runner) hands plain data back to
the UI thread and constructs commands there. Adding `Send` later is a
deliberate change, not an oversight.

## Invariants

1. Atomicity: if `Command::run` returns `Err`, `cx` is byte-for-byte as it
   was. The host then pushes nothing and leaves the redo side intact.
2. Completeness: any change to undoable state made by `run` is fully
   described by the returned `Edit`. A command that mutates undoable state
   and returns `Done` is a bug (M0.3 has no such state besides `theme`).
3. Replay: for any edit `e` returned by `run` from context `c0` leaving `c1`:
   `undo` on `c1` yields `c0`, and `redo` on `c0` yields `c1`, for any number
   of alternations. `redo` uses captured data only.
4. History is linear: `push` empties the redo side. A `Done` outcome does
   not touch either side.
5. Bounded: `undo_len() <= limit()` always; eviction drops the oldest.
6. `status_message` is not undoable state; `undo`/`redo` never touch it.
7. Registry ids are unique; enumeration order is ascending by id.
8. No panics: no `unwrap`/`expect`/indexing on untrusted lengths; empty
   stacks yield `Ok(false)`; unknown ids yield `Err(UnknownCommand)`.

## Test list

All in `crates/eda-cmd/tests/undo_model.rs` (public API only, so the tests
prove the surface `eda-ui` will use is sufficient), except the two marked
*unit* which live in `#[cfg(test)]` inside `builtins.rs` / `error.rs`.
Test-only helpers (`FailingCommand`, `FailingEdit`, `SetThemeCommand`) are
defined in the test file, not in the crate.

| Test | Proves |
| --- | --- |
| `command_id_display_eq_and_order` | `CommandId::new("a.b")` displays as `a.b`, equals `CommandId::from("a.b".to_string())`, and `app.hello < edit.redo < edit.undo < view.toggle_theme`. |
| `registry_lists_builtins_sorted_by_id` | `CommandRegistry::with_builtins().iter()` yields exactly `app.hello, edit.redo, edit.undo, view.toggle_theme` in that order; every `title()` is non-empty. |
| `registry_rejects_duplicate_id` | registering a second `HelloCommand` returns `Err(DuplicateId("app.hello"))` and `len()` is unchanged. |
| `registry_get_unknown_is_none` | `get(&CommandId::new("nope"))` is `None`. |
| `host_run_unknown_id_is_error_and_changes_nothing` | `run` returns `Err(UnknownCommand)`; context equals `Default`; `can_undo()` false. |
| `hello_sets_status_and_is_not_undoable` | after `run(app.hello)`: `status_message == Some(HELLO_MESSAGE)`, `can_undo()` false, `undo()` is `Ok(false)`. |
| `toggle_theme_flips_and_undo_restores` | Dark -> Light after run; `undo()` is `Ok(true)` and theme is Dark; `can_redo()` true. |
| `undo_then_redo_returns_identical_state` | run toggle twice, snapshot `context().clone()`, undo twice, redo twice; `context() == snapshot`. (The M1.1 acceptance wording, applied to the state that exists today.) |
| `theme_edit_redo_sets_captured_target_not_toggle` (*unit*) | `ThemeEdit::new(Dark, Light).redo` on a context already at Light leaves Light (a re-toggle would give Dark). Guards invariant 3. |
| `undo_and_redo_on_empty_stack_are_ok_false` | fresh host: `undo()` and `redo()` both `Ok(false)`, no error. |
| `new_edit_clears_redo` | toggle, undo, toggle: `can_redo()` false, `redo()` is `Ok(false)`. |
| `non_undoable_command_keeps_redo` | toggle, undo, hello: `can_redo()` still true; `redo()` is `Ok(true)` and theme is Light. Guards invariant 4's second sentence. |
| `undo_limit_evicts_oldest` | host with `UndoStack::with_limit(3)`, five toggles: `undo_len() == 3`; three undos return `Ok(true)`, fourth `Ok(false)`; theme is Dark (state after two toggles). |
| `undo_redo_commands_drive_the_stack` | `run(edit.undo)` / `run(edit.redo)` have the same effect on theme and stack as `undo()` / `redo()`. |
| `failed_command_is_not_recorded_and_keeps_redo` | set up one redo entry (toggle, undo); run a `FailingCommand`: result is `Err(Failed { id, .. })`, `undo_len() == 0`, `redo_len() == 1`, context unchanged. |
| `failed_undo_leaves_edit_on_undo_side` | push a `FailingEdit` via a test command; `undo()` is `Err(UndoFailed { label, .. })`; `can_undo()` still true, `can_redo()` false. |
| `undo_labels_follow_stack` | after toggle: `undo_label() == Some("Toggle theme")`, `redo_label() == None`; after undo: swapped. |
| `run_command_accepts_unregistered_values` | a test `SetThemeCommand { to: Light }` (not registered) run via `run_command` sets the theme and is undoable. Proves the M1.5 path (commands with arguments) needs no registry entry. |
| `undo_stack_matches_linear_history_model` | 2 000 steps from a hand-rolled LCG (no new dep) over {toggle, undo, redo, hello}; after each step the theme equals a reference model (`Vec<Theme>` of states + cursor; toggle truncates after cursor and pushes; undo/redo move the cursor; hello does nothing). Default limit, so no eviction. |
| `command_error_display_is_readable` (*unit*) | each variant's `Display` contains the id/label it names. |

No fuzz target: there is no parser in this crate. No property-test
dependency is added; the model test above covers the sequence space with
a deterministic generator.

## Edge cases the implementer must handle

- Borrowing in `CommandHost::run`: `self.registry.get(id)` borrows
  `self.registry` while `cmd.run(&mut self.cx)` borrows `self.cx`; that is
  fine as disjoint field borrows inside one method, but `run` must **not**
  delegate to `run_command(&mut self, ...)` while holding the registry
  reference. Compute the `Outcome` in an inner block, then apply it through
  a private `fn apply(&mut self, outcome)` shared by both entry points. Do
  not "fix" the borrow error by cloning commands.
- `Outcome::Undo`/`Outcome::Redo` from `run_command` of an arbitrary value
  behave exactly as from the builtins.
- Eviction with `limit == 1`: every push replaces the single entry.
- `clear()` empties both sides; `can_undo()`/`can_redo()` false after.
- `register` must not insert on duplicate (check before insert, or use
  `BTreeMap::entry`), so `len()` is unchanged on `Err`.
- Ids are not validated (empty, upper-case, spaces). They are developer
  constants in M0.3; validation can be added with scripting (Phase 4).
- `theme.toggled()` is self-inverse, which is why
  `theme_edit_redo_sets_captured_target_not_toggle` exists — the
  identical-state test alone cannot tell "replay `to`" from "toggle again".

## Ordered implementation checklist (one commit per step)

1. **Dependency sign-off.** Ask the owner to approve `thiserror` 2.x
   (crates.io; latest 2.0.21 on 2026-11-16, `MIT OR Apache-2.0`, MSRV 1.77
   which is under the workspace pin 1.82.0; reason: CLAUDE.md requires
   `thiserror` in libraries; this is the workspace's first dependency).
   Pin `thiserror = "2"` and let `Cargo.lock` fix the point release. Add it to
   `[workspace.dependencies]` in the root `Cargo.toml` and as
   `thiserror.workspace = true` in `crates/eda-cmd/Cargo.toml`. Commit the
   lockfile change. Do not proceed without the approval.
2. Create `crates/eda-cmd/tests/undo_model.rs` with every test in the list
   above (they will not compile yet); keep the existing `crate_name_is_set`
   test in `lib.rs`.
3. `id.rs`, `error.rs`, `context.rs` exactly as specified; `lib.rs` module
   declarations and re-exports. `cargo build -p eda-cmd` green.
4. `command.rs` (`Outcome`, `Command`, `Edit`).
5. `registry.rs` (`with_builtins` can call a not-yet-written
   `builtin_commands()`; stub it returning an empty `Vec` only within this
   step if needed, then replace in step 7).
6. `undo.rs`.
7. `builtins.rs` with the four commands, `ThemeEdit`, `builtin_commands()`,
   and the two unit tests.
8. `host.rs`. Run `cargo nextest run -p eda-cmd` until all tests pass.
9. `cargo clippy --workspace -- -D warnings`, `cargo fmt`, then
   `nix flake check`. Fix only what those report.
10. Report per CLAUDE.md; the `eda-ui` wiring (palette list from
    `registry().iter()`, `run(id)` on selection, re-sync the Slint theme
    property from `context().theme` after every `run`, keymap entries for
    `edit.undo`/`edit.redo`) is the Tier S part of M0.3 and is not covered
    by this note beyond that contract.

## What this does NOT solve yet

- **A real data model.** `CommandContext` has no design in it. M1.1 adds
  `pub design: eda_db::<Design>` (name decided there), implements `Edit` for
  `eda-db`'s patch/transaction type, and writes schematic commands. The
  `Command`/`Edit`/`UndoStack`/`CommandHost` signatures are meant to survive
  that unchanged; if M1.1 finds they cannot, it says so in its design note.
- **Compound edits.** Multi-step operations (M1.5 paste, M1.9 symbol from
  schematic) need a `CompoundEdit(Vec<Box<dyn Edit>>)` that undoes in reverse
  order. Trivial to add; not needed for M0.3.
- **Edit merging** (coalescing consecutive drags into one history entry),
  **named checkpoints**, and **per-document stacks** (several open views):
  M1.5 / M1.9 concerns. The stack is per `CommandHost`; one host per
  document is the expected shape then.
- **Command arguments from the palette or scripts.** Palette commands take
  everything from `cx`; argument-carrying commands are constructed in Rust.
  A string/argument parsing layer is Phase 4.
- **Enablement / visibility** (`is_enabled(&cx)` for greying out palette
  entries) and **keybinding hints in titles**: UI concerns for a later
  milestone; the keymap file is `eda-ui`'s.
- **Multiple named themes.** `Theme` is a two-variant enum because M0.3's
  check is "theme switch works". If the token file grows more themes, a
  `view.set_theme` command and a richer `Theme` replace `toggled()`.

## Not verified

- `thiserror` version, license and MSRV were read from the crates.io API on
  2026-11-16 (2.0.21, `MIT OR Apache-2.0`, MSRV 1.77); not built against
  the workspace yet.
- That Slint's event loop makes `Rc<RefCell<CommandHost>>` the convenient
  ownership shape in `eda-ui`; stated as expectation, decided in the Tier S
  part of M0.3.
- Nothing was compiled: this note contains signatures only.
