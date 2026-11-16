// SPDX-License-Identifier: GPL-3.0-or-later

//! Fuzz target for `eda_ui::Keymap::parse` (`crates/eda-ui/src/keymap.rs`).
//!
//! CLAUDE.md: "Parsers must never panic on bad input. Each parser gets a
//! fuzz target." The unit tests in `keymap.rs` already throw a handful of
//! malformed shapes at the parser by hand; this target lets `cargo fuzz
//! run keymap` explore the input space instead, the same way `eda-xschem`
//! (M1.2) will need to for its own file-format parser.
//!
//! libFuzzer hands us arbitrary bytes, which are not guaranteed to be
//! valid UTF-8. `Keymap::parse` takes `&str`, so invalid UTF-8 is handled
//! right here rather than inside the parser: `str::from_utf8` returning
//! `Err` is a normal, graceful "skip this input" outcome, not a panic.
//! The only thing this target asserts is that calling `Keymap::parse`
//! never panics -- a successful `Ok`/`Err` result is not inspected,
//! since any input the parser accepts or rejects without unwinding is
//! correct behaviour for a fuzz target (CLAUDE.md names only "never
//! panics on bad input" as the requirement).

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = eda_ui::Keymap::parse(text);
});
