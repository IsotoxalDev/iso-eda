// SPDX-License-Identifier: GPL-3.0-or-later

/// Generic string-keyed, string-valued, **ordered**, duplicate-permitting
/// attribute bag. `eda-db` assigns no meaning to any key — xschem's
/// `name=`/`type=`/`format=` conventions (M1.2) and M1.6's `format`
/// attribute are just entries here, nothing more.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttributeBag {
    entries: Vec<(String, String)>,
}

impl AttributeBag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends; does not deduplicate.
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// First match in insertion order.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// All matches, in insertion order.
    pub fn get_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.entries
            .iter()
            .filter(move |(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Removes every entry with this key; returns how many were removed.
    pub fn remove_all(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Convenience for "the key has exactly one current value": removes
    /// every existing entry with `key`, then appends once. NOTE: this does
    /// NOT preserve the removed entry's original position (it moves to the
    /// end) — flagged as a lossless-round-trip risk for M1.2, not solved here.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.remove_all(&key);
        self.entries.push((key, value.into()));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
