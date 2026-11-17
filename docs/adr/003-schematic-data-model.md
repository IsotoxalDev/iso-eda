# ADR-003: Schematic data model (`eda-geom`, `eda-db`) and the first schematic commands

- Status: accepted (2026-11-17, owner approved the design and the
  `proptest` dependency sign-off below; implemented and reviewed the same day)
- Date: 2026-11-17
- Milestone: M1.1 — `eda-geom`, `eda-db`, `eda-cmd`: library/cell/view model,
  schematic objects, command + undo stack (Tier F per `docs/MILESTONES.md`
  and `docs/MODEL-ROUTING.md` rubric points 1, 3, 5: new public types in
  `eda-db`/`eda-geom`, undo correctness is explicitly flagged as subtle,
  and this note asks for a new dependency, `proptest`)

## Context

CLAUDE.md: "Geometry uses integer database units. No floats in `eda-db`."
`docs/PLAN.md` targets xschem `.sch`/`.sym` as the schematic format
(M1.2, lossless round-trip) and a generic attribute bag so M1.2's xschem
attributes and M1.6's `format` netlisting attribute slot in without
`eda-db` knowing anything about xschem. M1.1's acceptance check is "Unit +
property tests; undo then redo returns identical state" — the exact
property ADR-002 flagged as the one subtlety that matters: **redo must
replay a captured change, including any id allocated the first time, never
recompute or reallocate**, or a later edit referencing that id silently
targets the wrong object after an undo/redo cycle.

ADR-002 already built `Command`, `Edit`, `Outcome`, `CommandRegistry`,
`UndoStack`, `CommandHost` in `eda-cmd` and said: "M1.1 adds
`pub design: eda_db::<Design>` here; nothing else about this type changes"
and "implements `Edit` for `eda-db`'s patch/transaction type, and writes
schematic commands." This note keeps that promise: no signature in
`command.rs`, `registry.rs`, `undo.rs`, `host.rs`, or `builtins.rs` changes.
One clarification to ADR-002's phrasing: the dependency direction
(`geom -> db -> ... -> cmd -> ...`) means `eda-db` cannot depend on
`eda-cmd`, so the type that implements `eda_cmd::Edit` cannot live in
`eda-db` — it is defined in `eda-cmd` itself, built on top of plain
`eda-db` data and id-allocator methods. `eda-db` stays pure data with no
knowledge that `eda-cmd` exists. This is not a deviation from ADR-002 (it
never said which crate owns the patch type), just the dependency rule
applied.

Constraints carried over: no `unsafe`; no `unwrap`/`expect` outside tests;
`thiserror` in libraries, but only where something is actually fallible;
new dependencies need sign-off; the host is single-threaded, no `Send`/
`Sync` bounds needed. M1.1 is explicitly scoped to a **flat, single
schematic view** being edited by a **single `CommandHost`** (ADR-002's "one
host per document is the expected shape"). Full editing (select, rotate,
mirror, rubber-band, copy/paste), rendering, netlisting, hierarchy, and
xschem-specific semantics are later milestones (M1.2, M1.4, M1.5, M1.6,
M1.9) and must not be invented here.

## Options considered

**Scalar width for `eda-geom`'s database unit.** `i32` (plain alias, as the
M0.2 spikes already used for a `[0, 1_000_000]` world) vs `i64` vs a
newtype wrapper. Schematics are component-scale (M0.2's spike world is
already a generous proxy); layout-scale coordinates (nanometres over
multi-centimetre dies) are `eda-gds`'s problem in Phase 3, a different
crate that is free to pick a wider or scaled type then. A newtype adds
`Add`/`Sub`/`Neg` boilerplate with no behaviour `i32` doesn't already have
for this milestone. **Chosen: `pub type Unit = i32`,** revisited only if
`eda-gds` needs more and decides it needs it in `eda-geom` rather than its
own crate.

**One `Id<T>` generic vs five hand-written id newtypes
(`InstanceId`, `WireId`, `LabelId`, `PinId`, ...).** Five newtypes are
maximally explicit but are five copies of the same ten lines, with no
behavioural difference from a generic. A raw `u64`/`usize` everywhere would
let a command accidentally pass a `WireId` where an `InstanceId` is
expected — exactly the kind of silent, subtle bug CLAUDE.md flags.
**Chosen: one generic `Id<T>` (phantom-typed), aliased per kind**
(`type InstanceId = Id<Instance>`), giving type safety with one
implementation to audit.

**Net membership: stored explicitly vs derived.** PLAN lists `Net` as a
schematic object, but a `Net { name, members: Vec<...> }` struct would have
to be kept in sync with every wire/instance edit by hand (a second source
of truth) or be rebuilt on every edit (nothing in M1.1 needs that cost).
Real connectivity (what counts as "touching", snap grids, label-driven
net names) is xschem/Phase-1-specific semantics that CLAUDE.md's rubric
marks "always Fable" and PLAN defers to M1.6. **Chosen: no `Net` type ships
in M1.1.** Net membership is derived from geometry (wire endpoints, resolved
instance-pin positions, label text) by a connectivity pass that does not
exist yet; `eda-db` only stores the primitives that pass will read. This is
stated as the explicit source-of-truth decision the task asked for.

**One generic inverse-`Patch` type vs a closed set of per-operation `Edit`
structs vs an open set (one struct per command, growing forever).** A
fully generic diff (old/new as type-erased blobs) would need downcasting
or serialization to compare/replay — against CLAUDE.md's "no unsafe,
correctness must not be subtle" and against "no new dependency" (would
likely want `serde`/`Any`). An open set of structs (`AddInstanceEdit`,
`MoveInstanceEdit`, ...) is maximally transparent (each type trivially
correct, mirrors ADR-002's `ThemeEdit`) but means a new public type and a
new `impl Edit` for every future operation forever, which will multiply
fast once M1.5 adds rotate/mirror/select/paste. **Chosen: a single
`pub(crate) enum Patch`** in `eda-cmd` with one variant per operation this
milestone needs (closed today, `pub(crate)` so adding a variant next
milestone is not a public API break) and **one `impl Edit for Patch`**
whose `undo`/`redo` match on the variant. Each match arm is exactly as
small and auditable as a dedicated struct would be; the enum just avoids
one-trait-impl-per-operation ceremony. This keeps ADR-002's expectation
("a `Patch` type can implement [`Edit`] once") while keeping each case
trivially correct, which is the deciding factor over the open-set option.

**`AttributeBag`: ordered multimap vs `HashMap`/`BTreeMap` (no
duplicates).** M1.2 needs lossless round-trip of xschem attribute strings,
which are order-sensitive and are not guaranteed duplicate-free by every
file in the wild. A map would force a decision (first/last wins, dropped
order) that M1.1 has no basis to make and that M1.2 would then have to
undo. **Chosen: an ordered `Vec<(String, String)>`,** duplicates allowed,
insertion order preserved; M1.2 inherits a structure that cannot lose
information it has not yet been taught to interpret.

## Decision

- `eda-geom` gains `Unit = i32`, `Point`, `Rect` (axis-aligned,
  `x0 <= x1 && y0 <= y1` by construction), `Orientation` (the 8-element
  dihedral group: four 90-degree rotations, four mirror-then-rotate
  combinations), and `Transform` (`Orientation` + translation). All
  arithmetic saturates; nothing in this crate can panic.
- `eda-db` gains `Id<T>`/`IdAllocator<T>`, `AttributeBag`, and the plain
  data types `Instance`, `Wire`, `Label`, `Pin`, `SchematicView`,
  `SymbolView`, `View`/`ViewKind`, `Cell`, `Library`, `Design`. Every
  mutating method on these is total (returns `Option`, never panics,
  never needs its own error enum — there is nothing fallible to report at
  this level, the same reasoning ADR-001's `spikes/scene` used for
  skipping `thiserror`). `IdAllocator<T>::alloc` is monotonic and is the
  **only** way an id is produced; undo/redo never call it.
- `eda-cmd` gains a `pub design: eda_db::Design` field on `CommandContext`
  (nothing else about that type changes), four new commands
  (`AddInstanceCommand`, `MoveInstanceCommand`, `DeleteInstanceCommand`,
  `AddWireCommand`), a small `SchematicEditError` (`thiserror`, used only
  as the boxed `source` of existing `CommandError` variants), and the
  `Patch` enum implementing `Edit`. None of the four commands is added to
  `CommandRegistry::with_builtins()` — they are run via
  `CommandHost::run_command`, the path ADR-002 already built and tested
  for "commands with arguments" — because palette/keymap wiring for
  editing is M1.5's job, not M1.1's.
- `proptest` is added as a dev-dependency (sign-off below) to satisfy the
  acceptance check's "property tests" wording directly, in `eda-geom`,
  `eda-db`, and `eda-cmd`.

## Consequences

Easier: M1.4 (rendering) and M1.5 (editing) consume `Point`/`Rect`/
`Transform` unchanged; M1.2 (xschem) consumes `AttributeBag` and
`Instance`/`Wire`/`Label`/`Pin` unchanged and only has to decide *how* to
fill and interpret them, not change their shape; M1.6 (netlisting) decides
connectivity without `eda-db` having guessed wrong first. `CommandContext`
keeps deriving `Eq` for free, because every new type is plain, integer,
and float-free — direct payoff of the "no floats in `eda-db`" rule.

Harder: every schematic command must be written as "capture old state,
mutate, return a `Patch`" (same discipline ADR-002 already imposed via
`Edit`); `SchematicView`/`SymbolView` carry a moderate amount of near-
identical boilerplate (alloc/insert/remove/get/get_mut/iter per object
kind) because no macro is used — a deliberate simplicity-over-cleverness
choice, not an oversight.

Reversal: `Patch` is `pub(crate)`, so switching to per-operation structs
later costs nothing outside `eda-cmd`. The `Id<T>` generic, if it proves
wrong, is one file to replace with hand-written newtypes; every call site
already looks like it would with real newtypes (`InstanceId`, not `u64`).

## Public types and traits (signatures only; the implementer builds from this)

### `eda-geom`

Crate layout: `crates/eda-geom/src/{lib.rs, unit.rs, point.rs, rect.rs,
transform.rs}`. `lib.rs` re-exports everything at the crate root. Every
file starts with the SPDX line. All arithmetic in this crate uses
saturating operations (`saturating_add`/`saturating_sub`; negation via
`0.saturating_sub(x)`, never plain `-x`, because negating `Unit::MIN` with
`-x` overflows) — stated once here, not repeated per method: **nothing in
`eda-geom` panics.**

```rust
// unit.rs
/// Integer database unit. No floats anywhere in this crate or in `eda-db`.
/// Chosen as a plain alias, not a newtype: schematics are component-scale;
/// layout-scale coordinates are `eda-gds`'s problem (Phase 3), in its own
/// crate, free to pick something wider.
pub type Unit = i32;

// point.rs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point { pub x: Unit, pub y: Unit }
impl Point {
    pub const ORIGIN: Point;
    pub fn new(x: Unit, y: Unit) -> Self;
    /// Saturating in both axes.
    pub fn translated(self, dx: Unit, dy: Unit) -> Self;
}

// rect.rs
/// Axis-aligned. Invariant: `x0 <= x1 && y0 <= y1`, always, by
/// construction — there is no way to build an invalid `Rect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect { pub x0: Unit, pub y0: Unit, pub x1: Unit, pub y1: Unit }
impl Rect {
    /// Normalizes (swaps) so the invariant holds; never panics.
    pub fn new(x0: Unit, y0: Unit, x1: Unit, y1: Unit) -> Self;
    /// `Rect::new` from two opposite corners in either order.
    pub fn from_points(a: Point, b: Point) -> Self;
    pub fn width(&self) -> Unit;   // x1 - x0, always >= 0
    pub fn height(&self) -> Unit;  // y1 - y0, always >= 0
    /// Smallest rect containing both inputs.
    pub fn union(&self, other: &Rect) -> Rect;
    /// Edge-inclusive: a point on the boundary counts as contained.
    pub fn contains_point(&self, p: Point) -> bool;
    /// Edge-inclusive: rects that only touch at an edge/corner intersect.
    pub fn intersects(&self, other: &Rect) -> bool;
    pub fn translated(&self, dx: Unit, dy: Unit) -> Rect;
}

// transform.rs
/// The 8-element dihedral group D4: four 90-degree rotations and their
/// four mirror-then-rotate combinations. Named, enumerable, and total —
/// this is deliberately the shape M1.2 needs to map xschem's `rot`
/// (0..=3 quarter turns) and `flip` (0/1) encoding onto; that mapping is
/// NOT decided here (see "Not solved yet" — `FlipR0` meaning "negate x,
/// then rotate 0" is this crate's own convention, not assumed to match
/// xschem's flip axis).
///
/// `apply_vector` is the linear part only (no translation):
/// - `R0(x,y) = (x,y)`
/// - `R90(x,y) = (-y,x)`
/// - `R180(x,y) = (-x,-y)`
/// - `R270(x,y) = (y,-x)`
/// - `FlipR0(x,y) = (-x,y)`
/// - `FlipR90(x,y) = (-y,-x)`
/// - `FlipR180(x,y) = (x,-y)`
/// - `FlipR270(x,y) = (y,x)`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default] R0, R90, R180, R270,
    FlipR0, FlipR90, FlipR180, FlipR270,
}
impl Orientation {
    pub fn apply_vector(self, x: Unit, y: Unit) -> (Unit, Unit);
    /// Group composition: "apply `self`, then `next`" as one `Orientation`.
    /// `self.compose(next).apply_vector(v) == next.apply_vector(self.apply_vector(v))`
    /// (component-wise on the tuple `v`).
    pub fn compose(self, next: Orientation) -> Orientation;
    /// The two-sided inverse under `compose`: `o.compose(o.inverse())
    /// == Orientation::R0`.
    pub fn inverse(self) -> Orientation;
    /// `true` for the four `FlipR*` variants.
    pub fn is_mirrored(self) -> bool;
}

/// Rigid (orthogonal) transform: linear part (`orientation`) applied
/// first, then `translation` added. No shear, no scale — not needed for
/// schematics, and deliberately not building layout's general affine case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Transform { pub orientation: Orientation, pub translation: Point }
impl Transform {
    pub const IDENTITY: Transform;
    pub fn new(orientation: Orientation, translation: Point) -> Self;
    pub fn translation(dx: Unit, dy: Unit) -> Self; // orientation = R0
    /// `apply_point(p) = orientation.apply_vector(p) + translation`.
    pub fn apply_point(&self, p: Point) -> Point;
    /// Transforms all four corners and re-normalizes via `Rect::from_points`
    /// + `union`. Always axis-aligned again: every `Orientation` is a
    /// 90-degree-step map.
    pub fn apply_rect(&self, r: Rect) -> Rect;
    /// `self.then(next)` means "apply `self` first, then `next`":
    /// `self.then(next).apply_point(p) == next.apply_point(self.apply_point(p))`.
    /// Formula the implementer must use (do not re-derive a different one):
    /// `orientation = self.orientation.compose(next.orientation)`;
    /// `translation = next.orientation.apply_vector(self.translation) + next.translation`
    /// (component-wise, saturating).
    pub fn then(&self, next: &Transform) -> Transform;
    /// `t.then(&t.inverse()) == Transform::IDENTITY` for every `t`.
    /// Formula: `inv_orientation = self.orientation.inverse()`;
    /// `inv_translation = -inv_orientation.apply_vector(self.translation)`
    /// (negate via `0.saturating_sub`, component-wise).
    pub fn inverse(&self) -> Transform;
}
```

### `eda-db`

Crate layout: `crates/eda-db/src/{lib.rs, id.rs, attr.rs, instance.rs,
wire.rs, label.rs, pin.rs, view.rs, cell.rs, library.rs, design.rs}`.
`eda-db` depends on `eda-geom` only. No `thiserror` dependency is added:
every mutating method below is total (`Option`-returning, never panics);
there is nothing fallible to name an error for at this layer (the same
reasoning ADR-001 used to skip `thiserror` in `spikes/scene`, "its
functions are total").

```rust
// id.rs
/// Typed id, phantom-tagged by the kind of object it names. Manually
/// implemented (not `#[derive]`, which would wrongly require `T: Trait`)
/// using `PhantomData<fn() -> T>` so no bound on `T` leaks out.
pub struct Id<T> { /* value: u64, _kind: PhantomData<fn() -> T> */ }
impl<T> Id<T> {
    pub fn as_u64(self) -> u64;
}
// Manual impls, none requiring `T: Trait`: Clone, Copy, Debug, PartialEq,
// Eq, Hash, PartialOrd, Ord (comparing only the `u64` value).

/// Monotonic counter. `alloc` never returns a value it has already
/// returned, for the lifetime of this allocator — including across
/// deletes, undo, and redo. This is the one mechanism that makes "redo
/// reproduces the same id" automatic rather than something every command
/// author must remember: commands call `alloc`; `Patch::undo`/`redo` never do.
pub struct IdAllocator<T> { /* next: u64, _kind: PhantomData<fn() -> T> */ }
impl<T> IdAllocator<T> {
    pub fn new() -> Self; // first alloc() returns Id(0)
    pub fn alloc(&mut self) -> Id<T>; // next = next.wrapping_add(1) after
}

pub type InstanceId = Id<Instance>;
pub type WireId = Id<Wire>;
pub type LabelId = Id<Label>;
pub type PinId = Id<Pin>;

// attr.rs
/// Generic string-keyed, string-valued, **ordered**, duplicate-permitting
/// attribute bag. `eda-db` assigns no meaning to any key — xschem's
/// `name=`/`type=`/`format=` conventions (M1.2) and M1.6's `format`
/// attribute are just entries here, nothing more.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributeBag { /* entries: Vec<(String, String)> */ }
impl AttributeBag {
    pub fn new() -> Self;
    /// Appends; does not deduplicate.
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>);
    /// First match in insertion order.
    pub fn get(&self, key: &str) -> Option<&str>;
    /// All matches, in insertion order.
    pub fn get_all<'a>(&'a self, key: &str) -> impl Iterator<Item = &'a str> + 'a;
    /// Removes every entry with this key; returns how many were removed.
    pub fn remove_all(&mut self, key: &str) -> usize;
    /// Convenience for "the key has exactly one current value": removes
    /// every existing entry with `key`, then appends once. NOTE: this does
    /// NOT preserve the removed entry's original position (it moves to the
    /// end) — flagged as a lossless-round-trip risk for M1.2, not solved here.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>);
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

// instance.rs
/// A placed reference to another `Cell`'s symbol, plus placement and
/// metadata. The instance's own name (xschem's `name=U1`) is NOT a typed
/// field — it lives in `attributes` like every other xschem attribute,
/// per the "don't invent xschem semantics here" rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    /// Name of the referenced `Cell` within the same `Library`. Not
    /// validated or resolved here (M1.6/M1.9's job); loading or editing a
    /// schematic with a dangling reference must not panic.
    pub cell_name: String,
    pub transform: eda_geom::Transform,
    pub attributes: AttributeBag,
}

// wire.rs
/// One straight net segment — matching xschem's model, not a general
/// polyline. Zero-length (`p0 == p1`) is legal; nothing here rejects it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wire {
    pub p0: eda_geom::Point,
    pub p1: eda_geom::Point,
    pub attributes: AttributeBag,
}

// label.rs
/// Free text anchored at a point — serves both plain annotation and
/// net-name labelling; which role a given `Label` plays is xschem/M1.2
/// semantics, not decided here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub position: eda_geom::Point,
    pub text: String,
    pub attributes: AttributeBag,
}

// pin.rs
/// A symbol-level connection point, in the symbol's own local coordinate
/// frame (before any instance `Transform` is applied).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub name: String,
    pub position: eda_geom::Point,
    pub attributes: AttributeBag,
}

// view.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViewKind { Schematic, Symbol }

/// Holds the `Instance`/`Wire`/`Label` objects of one schematic and the
/// three id allocators that own their ids. No `Net` type: net membership
/// is derived from this data by a connectivity pass that does not exist
/// yet (M1.6); this struct only stores what that pass will read.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SchematicView { /* BTreeMap<InstanceId, Instance>, BTreeMap<WireId, Wire>,
                               BTreeMap<LabelId, Label>, and one IdAllocator<T> per kind */ }
impl SchematicView {
    pub fn new() -> Self;
    pub fn alloc_instance_id(&mut self) -> InstanceId;
    /// Returns the previous occupant of `id`, if any (should normally be
    /// `None`; `Some` signals the caller reused an id, which is a bug).
    pub fn insert_instance(&mut self, id: InstanceId, instance: Instance) -> Option<Instance>;
    pub fn remove_instance(&mut self, id: InstanceId) -> Option<Instance>;
    pub fn instance(&self, id: InstanceId) -> Option<&Instance>;
    pub fn instance_mut(&mut self, id: InstanceId) -> Option<&mut Instance>;
    pub fn instances(&self) -> impl Iterator<Item = (InstanceId, &Instance)>; // ascending by id
    // Identical pattern for wires: alloc_wire_id, insert_wire, remove_wire,
    // wire, wire_mut, wires.
    // Identical pattern for labels: alloc_label_id, insert_label,
    // remove_label, label, label_mut, labels.
}

/// Holds the `Pin` list of one symbol. No graphic primitives (outline,
/// lines, arcs) yet — those are M1.2 (parsing) / M1.4 (rendering) territory.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SymbolView { /* BTreeMap<PinId, Pin>, IdAllocator<Pin> */ }
impl SymbolView {
    pub fn new() -> Self;
    pub fn alloc_pin_id(&mut self) -> PinId;
    pub fn insert_pin(&mut self, id: PinId, pin: Pin) -> Option<Pin>;
    pub fn remove_pin(&mut self, id: PinId) -> Option<Pin>;
    pub fn pin(&self, id: PinId) -> Option<&Pin>;
    pub fn pin_mut(&mut self, id: PinId) -> Option<&mut Pin>;
    pub fn pins(&self) -> impl Iterator<Item = (PinId, &Pin)>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View { Schematic(SchematicView), Symbol(SymbolView) }
impl View {
    pub fn kind(&self) -> ViewKind;
    pub fn as_schematic(&self) -> Option<&SchematicView>;
    pub fn as_schematic_mut(&mut self) -> Option<&mut SchematicView>;
    pub fn as_symbol(&self) -> Option<&SymbolView>;
    pub fn as_symbol_mut(&mut self) -> Option<&mut SymbolView>;
}

// cell.rs
/// At most one view per `ViewKind` (the Cadence/xschem cellview
/// convention: a cell has a `.sch` and/or a `.sym`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Cell { pub name: String, /* views: BTreeMap<ViewKind, View> */ }
impl Cell {
    pub fn new(name: impl Into<String>) -> Self;
    /// Keyed by `view.kind()`; returns the previous view of that kind, if any.
    pub fn insert_view(&mut self, view: View) -> Option<View>;
    pub fn remove_view(&mut self, kind: ViewKind) -> Option<View>;
    pub fn schematic(&self) -> Option<&SchematicView>;
    pub fn schematic_mut(&mut self) -> Option<&mut SchematicView>;
    pub fn symbol(&self) -> Option<&SymbolView>;
    pub fn symbol_mut(&mut self) -> Option<&mut SymbolView>;
}

// library.rs
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Library { pub name: String, /* cells: BTreeMap<String, Cell> */ }
impl Library {
    pub fn new(name: impl Into<String>) -> Self;
    /// Keyed by `cell.name.clone()`; returns the previous cell of that
    /// name, if any.
    pub fn insert_cell(&mut self, cell: Cell) -> Option<Cell>;
    pub fn remove_cell(&mut self, name: &str) -> Option<Cell>;
    pub fn cell(&self, name: &str) -> Option<&Cell>;
    pub fn cell_mut(&mut self, name: &str) -> Option<&mut Cell>;
    pub fn cells(&self) -> impl Iterator<Item = &Cell>; // ascending by name
}

// design.rs
/// The state one `CommandHost` edits: one `Library` plus which `Cell` is
/// open. M1.1's stack is per-document (ADR-002); multi-cell navigation
/// (descend/ascend) is M1.9's job.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Design { pub library: Library, pub active_cell: String }
impl Design {
    pub fn new(library: Library, active_cell: impl Into<String>) -> Self;
    /// `None` if `active_cell` does not name a cell, or that cell has no
    /// schematic view.
    pub fn active_schematic(&self) -> Option<&SchematicView>;
    pub fn active_schematic_mut(&mut self) -> Option<&mut SchematicView>;
}
```

### `eda-cmd` additions

New file `crates/eda-cmd/src/schematic.rs`, re-exported from `lib.rs`.
`crates/eda-cmd/Cargo.toml` gains a path dependency on `eda-db` (an
intra-workspace dependency the architecture already allows; not a
CLAUDE.md "new dependency" needing sign-off). `context.rs` changes by
exactly one field, as ADR-002 specified.

```rust
// context.rs — the only change to this file
pub struct CommandContext {
    pub theme: Theme,
    pub status_message: Option<String>,
    pub design: eda_db::Design,
}

// schematic.rs
#[derive(Debug, thiserror::Error)]
pub enum SchematicEditError {
    #[error("no cell named `{0}` is active, or it has no schematic view")]
    NoActiveSchematic(String),
    #[error("instance {0:?} not found")]
    NoSuchInstance(eda_db::InstanceId),
    #[error("wire {0:?} not found")]
    NoSuchWire(eda_db::WireId),
}

/// The recorded change for all four schematic commands below. `pub(crate)`:
/// not part of the public API outside `eda-cmd`, only `Outcome::Edit(Box::new(..))`
/// crosses the boundary, as a `Box<dyn Edit>`.
pub(crate) enum Patch {
    AddInstance { id: eda_db::InstanceId, instance: eda_db::Instance },
    DeleteInstance { id: eda_db::InstanceId, instance: eda_db::Instance },
    MoveInstance { id: eda_db::InstanceId, from: eda_geom::Transform, to: eda_geom::Transform },
    AddWire { id: eda_db::WireId, wire: eda_db::Wire },
}
impl Edit for Patch {
    fn label(&self) -> &str; // "Add instance" | "Delete instance" | "Move instance" | "Add wire"
    /// Every arm: look up `cx.design.active_schematic_mut()`; `None` or a
    /// missing id is `CommandError::UndoFailed { label: self.label().into(),
    /// source: Box::new(SchematicEditError::...) }`. Never recomputes:
    /// `AddInstance`/`AddWire` remove the id they recorded; `DeleteInstance`
    /// re-inserts the exact `instance` it captured at the exact `id`;
    /// `MoveInstance` sets the transform back to `from`.
    fn undo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
    /// Mirror of `undo`: `AddInstance`/`AddWire` re-insert at the captured
    /// id (never a fresh one); `DeleteInstance` removes the id again;
    /// `MoveInstance` sets the transform to `to` (not a re-derived value —
    /// same discipline as `ThemeEdit::redo` setting `to`, not toggling again).
    fn redo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
}

pub struct AddInstanceCommand { pub cell_name: String, pub transform: eda_geom::Transform, pub attributes: eda_db::AttributeBag }
impl Command for AddInstanceCommand {
    fn id(&self) -> CommandId; // "schematic.add_instance"
    fn title(&self) -> &str;   // "Add instance"
    /// Allocates a fresh `InstanceId` via `SchematicView::alloc_instance_id`,
    /// inserts, returns `Outcome::Edit(Box::new(Patch::AddInstance { id, instance }))`.
    /// `Err(Failed)` with `SchematicEditError::NoActiveSchematic` if there is none;
    /// `cx` is unchanged in that case (the allocator is never touched before
    /// the active-schematic check).
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}

pub struct MoveInstanceCommand { pub id: eda_db::InstanceId, pub to: eda_geom::Transform }
impl Command for MoveInstanceCommand {
    fn id(&self) -> CommandId; // "schematic.move_instance"
    fn title(&self) -> &str;   // "Move instance"
    /// Captures the current transform as `from`, sets it to `self.to`,
    /// returns `Outcome::Edit(Box::new(Patch::MoveInstance { id, from, to }))`.
    /// `Err(Failed(NoSuchInstance))` if `id` is not present; `cx` unchanged.
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}

pub struct DeleteInstanceCommand { pub id: eda_db::InstanceId }
impl Command for DeleteInstanceCommand {
    fn id(&self) -> CommandId; // "schematic.delete_instance"
    fn title(&self) -> &str;   // "Delete instance"
    /// Removes and captures the instance, returns
    /// `Outcome::Edit(Box::new(Patch::DeleteInstance { id, instance }))`.
    /// `Err(Failed(NoSuchInstance))` if `id` is not present; `cx` unchanged.
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}

pub struct AddWireCommand { pub wire: eda_db::Wire }
impl Command for AddWireCommand {
    fn id(&self) -> CommandId; // "schematic.add_wire"
    fn title(&self) -> &str;   // "Add wire"
    /// Allocates a fresh `WireId`, inserts, returns
    /// `Outcome::Edit(Box::new(Patch::AddWire { id, wire }))`.
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}
```

None of these four commands is registered in `CommandRegistry::with_builtins()`.
They are run through `CommandHost::run_command`, exactly the path ADR-002's
`run_command_accepts_unregistered_values` test already proves exists and
works for "commands with arguments". Palette entries and keymap bindings
for editing are M1.5's job.

## Invariants

1. No floats anywhere in `eda-geom` or `eda-db` (CLAUDE.md, unconditional).
2. Nothing in `eda-geom` panics: all `Unit` arithmetic saturates;
   `Rect::new`/`from_points` normalize instead of rejecting.
3. `Rect` invariant `x0 <= x1 && y0 <= y1` holds for every value that
   exists — there is no constructor that can violate it.
4. `Orientation`'s 8 variants are exactly the dihedral group of order 8
   under `compose`; `inverse` is a true two-sided inverse; `Transform::then`
   composes in the fixed, documented order ("self first, then the
   argument") — never ambiguous, never silently reversed.
5. `IdAllocator<T>::alloc` is strictly monotonic and never reused for the
   lifetime of the `SchematicView`/`SymbolView` that owns it, including
   across delete, undo, and redo. Only `Command::run` implementations call
   `alloc`; `Patch::undo`/`redo` only ever insert/remove/restore at an id
   they already captured.
6. `AttributeBag` preserves insertion order and permits duplicate keys;
   `get`/`get_all` never reorder; only `set` relocates an entry (documented).
7. No `Net` type exists; net membership is derived, not stored, in M1.1 —
   `eda-db` stores only the primitives a future connectivity pass needs.
8. `Patch` captures by value at the time `run`/`undo`/`redo` executes and
   never recomputes a captured field — same discipline as ADR-002's
   `Edit` invariant 3, now exercised by `Instance`/`Wire`/`Transform` state
   instead of only `Theme`.
9. `eda-db` has zero knowledge of `eda-cmd`; `eda-cmd` depends on `eda-db`
   and `eda-geom`; the dependency direction in CLAUDE.md is preserved.
10. `CommandContext` continues to derive `Clone, Debug, PartialEq, Eq,
    Default` — achievable only because every new type is plain and
    float-free; this is tested directly (see `design_round_trips_through_eq`).
11. No `unsafe`, no `unwrap`/`expect` outside tests, anywhere added by this
    milestone.

## Test list

### `crates/eda-geom/tests/geom.rs`

| Test | Proves |
| --- | --- |
| `point_translated_moves_by_delta` | `Point::new(1,2).translated(3,-5) == Point::new(4,-3)`. |
| `rect_new_normalizes_swapped_corners` | `Rect::new(10,10,0,0) == Rect::new(0,0,10,10)`; never panics on reversed input. |
| `rect_from_points_normalizes_either_order` | `Rect::from_points(a,b) == Rect::from_points(b,a)` for a fixed pair. |
| `rect_width_height_are_nonnegative` | for several rects, `width() >= 0 && height() >= 0`. |
| `rect_union_contains_both_inputs` | `union(a,b)` contains every corner of `a` and of `b`, for a fixed pair. |
| `rect_contains_point_is_edge_inclusive` | a point exactly on `x0`/`x1`/`y0`/`y1` is contained. |
| `rect_intersects_detects_touching_as_intersecting` | two rects sharing only an edge: `intersects` is `true`; two disjoint rects: `false`. |
| `orientation_apply_vector_matches_documented_table` | all 8 variants applied to `(2,3)` (and to `(0,0)`) match the table in the doc comment exactly. |
| `orientation_r90_four_times_is_r0` | `R90.compose(R90).compose(R90).compose(R90) == R0`. |
| `orientation_inverse_round_trips_for_all_eight` | for each of the 8 variants, `apply_vector` then `inverse().apply_vector` returns the original vector, checked on 2 sample points each. |
| `transform_identity_is_a_no_op` | `Transform::IDENTITY.apply_point(p) == p` for several `p`. |
| `transform_then_order_is_not_commutative` | a worked numeric example where `rotate.then(&translate) != translate.then(&rotate)`, pinning the documented order against accidental reversal. |
| `transform_inverse_undoes_apply_worked_example` | one hand-computed `(Orientation, translation, point)` triple, checked against `inverse().apply_point(apply_point(p)) == p`. |
| `prop_orientation_compose_matches_sequential_apply` (proptest) | for random `(o1, o2, x, y)`: letting `(x1,y1) = o1.apply_vector(x,y)`, `o1.compose(o2).apply_vector(x,y) == o2.apply_vector(x1,y1)`. |
| `prop_transform_then_matches_sequential_apply` (proptest) | for random `(t1, t2, p)`: `t1.then(&t2).apply_point(p) == t2.apply_point(t1.apply_point(p))`. |
| `prop_transform_inverse_round_trips` (proptest) | for random `(t, p)`: `t.inverse().apply_point(t.apply_point(p)) == p` and `t.then(&t.inverse()) == Transform::IDENTITY`. |
| `prop_rect_apply_transform_stays_normalized` (proptest) | for random `(rect, transform)`: `transform.apply_rect(rect)` still satisfies `x0 <= x1 && y0 <= y1`. |
| `prop_rect_union_is_commutative_and_contains_inputs` (proptest) | for random `(a, b)`: `a.union(&b) == b.union(&a)`, and every corner of `a`/`b` is contained in the union. |

### `crates/eda-db/tests/schematic_model.rs`

| Test | Proves |
| --- | --- |
| `id_allocator_never_repeats_across_five_allocs` | `IdAllocator::<Instance>::new()` called 5 times via `alloc()` yields 5 distinct ids. |
| `attribute_bag_preserves_order_and_duplicates` | `push("a","1"); push("b","2"); push("a","3")` then `iter()` yields exactly `[("a","1"),("b","2"),("a","3")]`. |
| `attribute_bag_get_returns_first_match` | with the bag above, `get("a") == Some("1")`; `get_all("a")` yields `["1","3"]`. |
| `attribute_bag_set_replaces_all_and_appends_once` | `set("a","9")` on the bag above leaves exactly one `"a"` entry, value `"9"`, at the end. |
| `attribute_bag_remove_all_reports_count` | `remove_all("a")` on the bag above returns `2` and leaves only `("b","2")`. |
| `library_insert_and_remove_cell_round_trip` | insert a cell, `cell("x")` finds it; `remove_cell("x")` returns it and a second `cell("x")` is `None`. |
| `cell_insert_view_keeps_other_kind` | insert a `Schematic` view then a `Symbol` view on the same `Cell`: both `schematic()` and `symbol()` are `Some`; inserting a second `Schematic` view returns the first one from `insert_view`. |
| `design_active_schematic_is_none_without_a_matching_cell_and_view` | fresh `Design::default()`: `active_schematic()` is `None`; same after setting `active_cell` to a name not present in the library; same if the named cell has only a `Symbol` view. |
| `schematic_view_instance_insert_then_remove_round_trips` | `alloc_instance_id` + `insert_instance` then `remove_instance` returns the same `Instance`; a second `remove_instance` on the same id is `None`. |
| `schematic_view_accepts_zero_length_wire` | a `Wire` with `p0 == p1` inserts and reads back without any special handling. |
| `design_round_trips_through_eq` | two `Design`s built via the identical sequence of `insert_cell`/`insert_view`/`insert_instance` calls compare equal with `==`; this is the property the M1.1 acceptance check (undo-then-redo identical state) ultimately depends on. |
| `prop_instance_ids_stay_unique_under_random_alloc_insert_remove` (proptest) | a random sequence of alloc/insert/remove on one `SchematicView` never leaves two live instances sharing an id, even when ids freed by `remove` are never reused by later `alloc`s. |
| `prop_attribute_bag_get_after_set_returns_the_set_value` (proptest) | for random `(key, value)` and an arbitrary pre-existing bag: after `set(key, value)`, `get(key) == Some(value)`. |

### `crates/eda-cmd/tests/schematic_commands.rs`

(Builds a `CommandHost` with `CommandContext { design: <a Design with one
Library, one Cell named "top" with an empty SchematicView, active_cell:
"top".into() >, ..Default::default() }`; test-only helper, not in the crate.)

| Test | Proves |
| --- | --- |
| `add_instance_allocates_and_is_undoable` | `run_command(&AddInstanceCommand{..})`: the instance exists afterward at some id; `can_undo()` is `true`; after `undo()` it is gone. |
| `add_instance_fails_cleanly_with_no_active_schematic` | same command against `CommandContext::default()` (empty `Design`): `Err(Failed{..})`; `cx.design` is unchanged (still `Design::default()`); `can_undo()` is `false`. |
| `move_instance_undo_restores_previous_transform` | add an instance, move it, `undo()`: its transform equals the pre-move value. |
| `move_instance_redo_sets_captured_target_not_a_relative_delta` | add an instance at `t0`, `MoveInstanceCommand{to: t1}`, `undo()` (back to `t0`), then directly call `Patch::redo` (via `host.redo()`): the transform is exactly `t1`, not `t0.then(&some_delta)`. Guards the same class of bug ADR-002's `theme_edit_redo_sets_captured_target_not_toggle` guards, now for a value (`Transform`) that is not self-inverse, so a wrong "redo re-applies a relative move" implementation is distinguishable from a correct "redo sets the captured absolute `to`" one. |
| `delete_instance_then_undo_reinserts_at_the_same_id` | add (captures `id`), delete, undo: the instance exists again at exactly `id` (checked via `SchematicView::instance(id)`, not just "some instance exists"). |
| `deleting_then_adding_never_reuses_the_freed_id` | add instance A (id `i0`), delete A, add instance B: B's id is **not** `i0` — the direct regression test for invariant 5. |
| `add_wire_undo_then_redo_restores_wire` | add a wire, undo (gone), redo (back, identical `Wire` value, same id). |
| `undo_then_redo_returns_identical_state_for_schematic_edits` | run `AddInstance`, `MoveInstance`, `AddWire` in sequence; snapshot `cx.design.clone()`; `undo()` three times; `redo()` three times; `cx.design == snapshot`. This is the acceptance check's exact wording, exercised on the new types (parallel to ADR-002's `undo_then_redo_returns_identical_state`). |
| `prop_schematic_edit_sequence_preserves_invariants` (proptest) | a random bounded-length sequence of `{AddInstance, MoveInstance(random live id), DeleteInstance(random live id), AddWire, undo, redo}` run through one `CommandHost`; after every step, asserts: all live instance ids are distinct, `undo_len() + redo_len()` only changes as ADR-002's linear-history model predicts, and that performing `undo()` immediately followed by `redo()` at any point leaves `cx.design` exactly as it was before that pair. |

## Edge cases the implementer must handle

- **Id reuse after delete never happens.** `IdAllocator::alloc` is the only
  source of new ids and is monotonic; `remove_instance`/`remove_wire`/
  `remove_label` never touch the allocator. Covered directly by
  `deleting_then_adding_never_reuses_the_freed_id`.
- **Undo of a delete, then redo.** Must restore then re-remove the exact
  captured object at the exact captured id — not "insert *an* instance",
  "the same `Instance` value at the same `InstanceId`". Covered by
  `delete_instance_then_undo_reinserts_at_the_same_id`.
- **Empty library / cell without a schematic view / unset `active_cell`.**
  `Design::active_schematic[_mut]` returns `None` in all three cases, with
  no distinction between them visible to callers; every command maps
  `None` to `Err(Failed(NoActiveSchematic))` and leaves `cx` untouched.
- **Duplicate attribute keys.** Legal; `get` returns the first; `get_all`
  returns all; `set` collapses to one at the end (documented, not "fixed",
  because xschem's own duplicate-key semantics are undecided — M1.2).
- **Degenerate (zero-length) wires.** Legal; not rejected by `Wire` or
  `insert_wire`; whether the UI should warn or snap-merge them is M1.5/M1.2.
- **Transform composition order.** `then` is "self first, argument
  second"; this is the single most likely place to introduce a silent,
  reversed-order bug — hence the worked numeric unit test
  (`transform_then_order_is_not_commutative`) in addition to the property test.
- **Negating `Unit::MIN`.** `Transform::inverse`'s translation negation
  must use `0.saturating_sub(x)`, not `-x`, or it overflows/panics in debug
  builds for the one pathological input `x == i32::MIN`.
- **`Orientation`'s mirror axis is this crate's own convention.** `FlipR0`
  means "negate x, then rotate by 0" — it is *not* assumed to match
  xschem's `flip` bit's axis. M1.2 must map xschem's `(rot, flip)` pair
  onto `Orientation` explicitly; this ADR does not do that mapping.
- **`Library::insert_cell` / `Cell::insert_view` silently replace.** They
  return the previous value via `Option` so a caller *can* notice, but
  nothing forces it to. None of M1.1's four commands calls these on a
  pre-existing name/kind, so the risk does not surface yet — noted for
  M1.2 (loading a library) and M1.9 (hierarchy) to handle deliberately.
- **A `Cell` whose active view is `Symbol`, not `Schematic`.** Treated
  identically to "no schematic view" — `active_schematic()` is `None`,
  same error path, no special case.

## Ordered implementation checklist (one commit per step)

1. **Dependency sign-off: `proptest`.** Ask the owner to approve `proptest`
   1.x (crates.io; latest found 1.12.0, released 2026-10-10, `MIT OR
   Apache-2.0`, MSRV 1.84 — comfortably under the workspace's
   `rust-toolchain.toml` pin of 1.98.1; reason: M1.1's acceptance check
   says "property tests" by name). Pin `proptest = "1"` in
   `[workspace.dependencies]`; add `proptest.workspace = true` under
   `[dev-dependencies]` in `crates/eda-geom/Cargo.toml`,
   `crates/eda-db/Cargo.toml`, and `crates/eda-cmd/Cargo.toml` (dev-only:
   nothing in library code needs it). Commit the lockfile change. Do not
   proceed without the approval.
2. `crates/eda-geom/tests/geom.rs` with every `eda-geom` test above (will
   not compile yet); keep `crate_name_is_set`.
3. `eda-geom`: `unit.rs`, `point.rs`, `rect.rs`. `cargo build -p eda-geom` green.
4. `eda-geom`: `transform.rs` (`Orientation`, `Transform`). `cargo nextest
   run -p eda-geom` green, all 17 tests passing.
5. `crates/eda-db/tests/schematic_model.rs` with every `eda-db` test above
   (will not compile yet); keep `crate_name_is_set`. Add `eda-geom` as a
   path dependency in `crates/eda-db/Cargo.toml` if not already present.
6. `eda-db`: `id.rs`, `attr.rs`.
7. `eda-db`: `instance.rs`, `wire.rs`, `label.rs`, `pin.rs`.
8. `eda-db`: `view.rs`, `cell.rs`, `library.rs`, `design.rs`. `cargo
   nextest run -p eda-db` green.
9. Add `eda-db` as a path dependency in `crates/eda-cmd/Cargo.toml`. Add
   `pub design: eda_db::Design` to `CommandContext` in `context.rs` (the
   one change ADR-002 pre-approved); confirm the existing `#[derive(...,
   PartialEq, Eq, Default)]` still compiles unchanged.
10. `crates/eda-cmd/tests/schematic_commands.rs` with every test above
    (will not compile yet); do not modify `tests/undo_model.rs`.
11. `eda-cmd`: `schematic.rs` (`SchematicEditError`, `Patch`,
    `AddInstanceCommand`, `MoveInstanceCommand`, `DeleteInstanceCommand`,
    `AddWireCommand`); re-export the four command types (and
    `SchematicEditError`, not `Patch`) from `lib.rs`. Do not touch
    `command.rs`, `registry.rs`, `undo.rs`, `host.rs`, `builtins.rs`. Run
    `cargo nextest run -p eda-cmd` until every test in both `undo_model.rs`
    and `schematic_commands.rs` passes.
12. `cargo clippy --workspace -- -D warnings`, `cargo fmt`, then
    `nix flake check`. Fix only what those report.
13. Report per CLAUDE.md; write `docs/manual-tests/M1.1.md` stating there
    is nothing to manually check yet — these commands are not reachable
    from any UI (palette, canvas, keymap) until M1.4/M1.5 — and why that
    is expected at this milestone.

## What this does NOT solve yet

- **Full editing operations.** Select, rotate, mirror, rubber-band,
  copy/paste, drag-preview, and merging consecutive edits into one history
  entry are M1.5. M1.1 ships exactly four standalone commands to exercise
  the `Edit` plumbing end to end, not a usable editor.
- **xschem-specific semantics.** The meaning of any attribute key
  (`name=`, `type=`, `format=`, ...), Tcl expression evaluation, and the
  `(rot, flip)` -> `Orientation` mapping are M1.2's job. `AttributeBag` and
  `Orientation` are deliberately ignorant of all of it.
- **Rendering.** No draw-list, scene graph, culling, or LOD exists.
  `Point`/`Rect`/`Transform` are shaped so M1.4 can consume them unchanged,
  but M1.4 still has to build everything on top.
- **Netlisting and connectivity.** No `Net` type, no notion of "touching",
  no snap grid. M1.6 decides all of this; `eda-db` only stores the
  geometry and text a connectivity pass will read.
- **Hierarchy beyond one flat schematic view.** Descend/ascend, resolving
  `Instance::cell_name` against a real symbol, multi-library references,
  and validating that a reference even exists are M1.9. `cell_name` is
  unresolved free text on purpose.
- **Compound/merged edits beyond the four `Patch` variants.** A
  `CompoundEdit(Vec<Box<dyn Edit>>)` for multi-step operations is still
  M1.5's addition, unchanged from ADR-002's "Not solved" list.
- **PDK symbols.** `SymbolView`/`Pin` exist structurally (so the type graph
  is complete and the id-allocator pattern is established uniformly) but
  no command in this milestone creates or edits a `Pin`, and no symbol
  graphics (outline, lines, arcs) exist. Populating them is M1.2/M1.3.
- **Multiple open documents.** `Design` is still "one active cell" shaped,
  matching ADR-002's "one host per document" expectation; switching cells
  within a `Library` without losing undo history is not designed here.

## Not verified

- `proptest`'s version (1.12.0), release date (2026-10-10), license
  (`MIT OR Apache-2.0`), and MSRV (1.84) were read via `WebSearch`/
  `WebFetch` against docs.rs on 2026-11-17; a direct `curl` to the
  crates.io API from this sandbox failed (no outbound network access), so
  this was not independently cross-checked against the crates.io API the
  way ADR-002 did for `thiserror`. Not built against the workspace.
- The `Orientation` composition/inverse formulas (`then`, `inverse`) were
  derived by hand in this note, not compiled or executed; the property
  tests (`prop_orientation_compose_matches_sequential_apply`,
  `prop_transform_then_matches_sequential_apply`,
  `prop_transform_inverse_round_trips`) exist specifically to catch an
  error in that derivation, which is the point of asking for `proptest`
  here rather than only hand-picked examples.
- Whether `i32` is generous enough for every schematic M1.2's real corpus
  will contain was not checked against actual xschem files or xschem's
  internal coordinate conventions; M1.2's format note should confirm
  before fixing the xschem-units-to-`Unit` mapping.
- The `(rot, flip)` -> `Orientation` mapping xschem actually needs was not
  checked against xschem source or sample `.sym`/`.sch` files; `Orientation`
  is this ADR's own convention, deliberately decoupled, and is explicitly
  flagged above as unresolved for M1.2 to settle.
- Nothing was compiled or run: this note contains signatures and doc-
  comment-level behavioural descriptions only, per instructions.
