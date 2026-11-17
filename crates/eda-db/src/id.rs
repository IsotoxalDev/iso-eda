// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Typed id, phantom-tagged by the kind of object it names. Manually
/// implemented (not `#[derive]`, which would wrongly require `T: Trait`)
/// using `PhantomData<fn() -> T>` so no bound on `T` leaks out.
pub struct Id<T> {
    value: u64,
    _kind: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    fn new(value: u64) -> Self {
        Self {
            value,
            _kind: PhantomData,
        }
    }

    pub fn as_u64(self) -> u64 {
        self.value
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Id").field(&self.value).finish()
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

/// Monotonic counter. `alloc` never returns a value it has already
/// returned, for the lifetime of this allocator — including across
/// deletes, undo, and redo. This is the one mechanism that makes "redo
/// reproduces the same id" automatic rather than something every command
/// author must remember: commands call `alloc`; `Patch::undo`/`redo` never do.
pub struct IdAllocator<T> {
    next: u64,
    _kind: PhantomData<fn() -> T>,
}

impl<T> IdAllocator<T> {
    /// First `alloc()` returns `Id(0)`.
    pub fn new() -> Self {
        Self {
            next: 0,
            _kind: PhantomData,
        }
    }

    pub fn alloc(&mut self) -> Id<T> {
        let id = Id::new(self.next);
        self.next = self.next.wrapping_add(1);
        id
    }
}

impl<T> Default for IdAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for IdAllocator<T> {
    fn clone(&self) -> Self {
        Self {
            next: self.next,
            _kind: PhantomData,
        }
    }
}

impl<T> fmt::Debug for IdAllocator<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdAllocator").field("next", &self.next).finish()
    }
}

impl<T> PartialEq for IdAllocator<T> {
    fn eq(&self, other: &Self) -> bool {
        self.next == other.next
    }
}

impl<T> Eq for IdAllocator<T> {}
