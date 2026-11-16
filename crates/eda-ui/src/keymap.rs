// SPDX-License-Identifier: GPL-3.0-or-later

//! Keymap file parsing: `key = command_id` pairs, grouped by preset file
//! (`keymap/xschem.keymap`, `keymap/virtuoso.keymap`). Pure and
//! display-independent so it is unit-testable without a GPU/display, and
//! never panics on malformed input (CLAUDE.md: "Parsers must never panic
//! on bad input").
//!
//! Format, one binding per line:
//! `<modifiers>+<key> = <command.id>`
//! Modifiers (any order, joined by `+`, case-insensitive): `ctrl`,
//! `shift`, `alt`, `meta`. `<key>` is matched case-insensitively against
//! the pressed key's text. Blank lines and lines starting with `#` are
//! ignored. A bare `<key>` with no modifiers is allowed (just the key
//! text before `=`).

use std::fmt;

use eda_cmd::CommandId;

/// A key combination: a set of modifier flags plus the (lowercased) key
/// text, e.g. `ctrl+shift+p`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyChord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub key: String,
}

/// One parsed line: a chord mapped to a command id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    pub chord: KeyChord,
    pub command: CommandId,
}

/// A parsed keymap: an ordered, duplicate-free set of bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<KeyBinding>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    #[error("line {line}: missing `=` separator")]
    MissingSeparator { line: usize },
    #[error("line {line}: empty key chord before `=`")]
    EmptyChord { line: usize },
    #[error("line {line}: unknown modifier `{modifier}`")]
    UnknownModifier { line: usize, modifier: String },
    #[error("line {line}: empty command id after `=`")]
    EmptyCommand { line: usize },
    #[error("line {line}: chord `{chord}` is already bound")]
    DuplicateChord { line: usize, chord: String },
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "ctrl+")?;
        }
        if self.shift {
            write!(f, "shift+")?;
        }
        if self.alt {
            write!(f, "alt+")?;
        }
        if self.meta {
            write!(f, "meta+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl Keymap {
    /// Parses a keymap file's text. Never panics: malformed input yields
    /// `Err`, naming the offending (1-based) line.
    pub fn parse(text: &str) -> Result<Keymap, KeymapError> {
        let mut bindings: Vec<KeyBinding> = Vec::new();

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((chord_part, command_part)) = line.split_once('=') else {
                return Err(KeymapError::MissingSeparator { line: line_no });
            };
            let chord_part = chord_part.trim();
            let command_part = command_part.trim();

            if chord_part.is_empty() {
                return Err(KeymapError::EmptyChord { line: line_no });
            }
            if command_part.is_empty() {
                return Err(KeymapError::EmptyCommand { line: line_no });
            }

            let chord = parse_chord(chord_part, line_no)?;

            if bindings.iter().any(|b: &KeyBinding| b.chord == chord) {
                return Err(KeymapError::DuplicateChord {
                    line: line_no,
                    chord: chord.to_string(),
                });
            }

            bindings.push(KeyBinding {
                chord,
                command: CommandId::from(command_part.to_owned()),
            });
        }

        Ok(Keymap { bindings })
    }

    /// Looks up the command bound to a pressed key. `text` is matched
    /// case-insensitively; modifier flags must match exactly.
    pub fn lookup(
        &self,
        text: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) -> Option<&CommandId> {
        let text = text.to_ascii_lowercase();
        self.bindings
            .iter()
            .find(|b| {
                b.chord.key == text
                    && b.chord.ctrl == ctrl
                    && b.chord.shift == shift
                    && b.chord.alt == alt
                    && b.chord.meta == meta
            })
            .map(|b| &b.command)
    }

    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

fn parse_chord(spec: &str, line_no: usize) -> Result<KeyChord, KeymapError> {
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut meta = false;

    // The key itself is the last `+`-separated part; everything before
    // it must be a known modifier name.
    let parts: Vec<&str> = spec.split('+').map(str::trim).collect();
    let Some((last, modifiers)) = parts.split_last() else {
        return Err(KeymapError::EmptyChord { line: line_no });
    };
    if last.is_empty() {
        return Err(KeymapError::EmptyChord { line: line_no });
    }
    let key = last.to_ascii_lowercase();

    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "meta" | "super" | "cmd" => meta = true,
            "" => return Err(KeymapError::EmptyChord { line: line_no }),
            other => {
                return Err(KeymapError::UnknownModifier {
                    line: line_no,
                    modifier: other.to_owned(),
                })
            }
        }
    }

    Ok(KeyChord {
        ctrl,
        shift,
        alt,
        meta,
        key,
    })
}

/// The xschem-style preset shipped with iso-eda.
pub const XSCHEM_PRESET: &str = include_str!("../keymap/xschem.keymap");
/// The Virtuoso-style preset shipped with iso-eda.
pub const VIRTUOSO_PRESET: &str = include_str!("../keymap/virtuoso.keymap");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_key() {
        let keymap = Keymap::parse("ctrl+shift+p = ui.open_palette\n").unwrap();
        assert_eq!(keymap.len(), 1);
        let binding = &keymap.bindings()[0];
        assert!(
            binding.chord.ctrl && binding.chord.shift && !binding.chord.alt && !binding.chord.meta
        );
        assert_eq!(binding.chord.key, "p");
        assert_eq!(binding.command.as_str(), "ui.open_palette");
    }

    #[test]
    fn modifier_names_and_key_are_case_insensitive() {
        let keymap = Keymap::parse("CTRL+Shift+P = ui.open_palette\n").unwrap();
        assert!(keymap.lookup("p", true, true, false, false).is_some());
        assert!(keymap.lookup("P", true, true, false, false).is_some());
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let keymap = Keymap::parse("# a comment\n\n  \nctrl+z = edit.undo\n").unwrap();
        assert_eq!(keymap.len(), 1);
    }

    #[test]
    fn bare_key_with_no_modifiers_is_allowed() {
        let keymap = Keymap::parse("h = app.hello\n").unwrap();
        assert_eq!(
            keymap
                .lookup("h", false, false, false, false)
                .unwrap()
                .as_str(),
            "app.hello"
        );
    }

    #[test]
    fn lookup_requires_exact_modifier_match() {
        let keymap = Keymap::parse("ctrl+z = edit.undo\n").unwrap();
        assert!(keymap.lookup("z", true, false, false, false).is_some());
        assert!(keymap.lookup("z", true, true, false, false).is_none());
        assert!(keymap.lookup("z", false, false, false, false).is_none());
    }

    #[test]
    fn missing_separator_is_an_error_not_a_panic() {
        let err = Keymap::parse("ctrl+z edit.undo\n").unwrap_err();
        assert_eq!(err, KeymapError::MissingSeparator { line: 1 });
    }

    #[test]
    fn empty_chord_is_an_error() {
        let err = Keymap::parse(" = edit.undo\n").unwrap_err();
        assert_eq!(err, KeymapError::EmptyChord { line: 1 });
    }

    #[test]
    fn empty_command_is_an_error() {
        let err = Keymap::parse("ctrl+z =\n").unwrap_err();
        assert_eq!(err, KeymapError::EmptyCommand { line: 1 });
    }

    #[test]
    fn unknown_modifier_is_an_error() {
        let err = Keymap::parse("hyper+z = edit.undo\n").unwrap_err();
        assert_eq!(
            err,
            KeymapError::UnknownModifier {
                line: 1,
                modifier: "hyper".to_owned()
            }
        );
    }

    #[test]
    fn duplicate_chord_is_an_error() {
        let err = Keymap::parse("ctrl+z = edit.undo\nctrl+z = edit.redo\n").unwrap_err();
        assert_eq!(
            err,
            KeymapError::DuplicateChord {
                line: 2,
                chord: "ctrl+z".to_owned()
            }
        );
    }

    #[test]
    fn modifier_order_does_not_matter_for_lookup() {
        let a = Keymap::parse("ctrl+shift+p = ui.open_palette\n").unwrap();
        let b = Keymap::parse("shift+ctrl+p = ui.open_palette\n").unwrap();
        assert_eq!(a.bindings()[0].chord, b.bindings()[0].chord);
    }

    #[test]
    fn empty_file_parses_to_empty_keymap() {
        let keymap = Keymap::parse("").unwrap();
        assert!(keymap.is_empty());
        assert!(keymap.lookup("z", true, false, false, false).is_none());
    }

    /// A cheap, fast guard that the parser returns `Err` rather than
    /// panicking on an assortment of malformed input shapes; the real
    /// fuzz target is `crates/eda-ui/fuzz/fuzz_targets/keymap.rs`.
    ///
    /// Known grammar limitation (not a panic risk): `+`, `=` and `#`
    /// cannot appear as the bound key, since `+` separates modifiers,
    /// `=` separates the chord from the command id, and a line starting
    /// with `#` is a comment. M1.3's zoom-in/out bindings will likely
    /// want `+`/`-`; revisit the grammar then rather than rediscovering
    /// this.
    #[test]
    fn parser_never_panics_on_malformed_input() {
        let long_line = "a".repeat(10_000);
        let inputs = [
            "\0\0\0",
            "=====",
            "++++ = x",
            "ctrl++ = x",
            "ctrl+ += x",
            "💥+ctrl = x",
            long_line.as_str(),
            "ctrl+z = edit.undo\0",
            "\n\n\n\n",
            "ctrl+z=edit.undo=extra",
        ];
        for input in inputs {
            let _ = Keymap::parse(input);
        }
    }

    #[test]
    fn shipped_presets_parse_and_contain_expected_commands() {
        for preset in [XSCHEM_PRESET, VIRTUOSO_PRESET] {
            let keymap = Keymap::parse(preset).unwrap();
            assert!(keymap.lookup("z", true, false, false, false).is_some());
            assert!(!keymap.is_empty());
        }
    }
}
