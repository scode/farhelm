//! Terminal input policy for the GPUI desktop app, without a GPUI dependency.
//!
//! This maps the fields GPUI actually exposes, not a browser event disguised as
//! native input. DOM key codes are inferred from named keys and US ASCII labels;
//! physical identity and Shift on symbol keys are not recoverable in general.
//! See the fixture audit (README.md) for the bounds of that inference. Terminal
//! byte encoding remains entirely in the supervisor.
//!
//! "Pinned" in this crate's comments means the old web client as it stood at the
//! migration's reference commit 24bc0580 (`grid-input.js`), whose decisions this
//! crate reproduces and whose tables its tests replay.

pub mod cursor;
mod keys;
pub mod pointer;

pub use keys::{
    Composition, Decision, KeyDown, Modifiers, NativeIdentity, NativeKey, Platform, Route, map_key,
};

use farhelm_proto::grid::{
    GRID_MAX_INPUT_MESSAGE, GRID_MAX_PASTE_PART_BYTES, GRID_MAX_TEXT_INPUT_BYTES, GridInput,
};

/// Split one committed insertion into valid `Text` messages, without losing text.
///
/// The owner calls this only for committed replacement, never for marked text.
/// A key mapped to `Send` must stop propagation before GPUI's automatic insertion;
/// otherwise that same character would be sent again here. There is deliberately
/// no time-based deduper: two identical commits can be two intentional keystrokes.
pub fn committed_text(text: &str) -> Vec<GridInput> {
    text_parts(text, GRID_MAX_TEXT_INPUT_BYTES, false)
        .into_iter()
        .map(|data| GridInput::Text { data })
        .collect()
}

/// Preserve one bracketed paste across valid wire parts, matching the pinned
/// grid-input.js policy. A CRLF pair stays together, so line-ending conversion
/// observes the same text regardless of the chunk boundary. Only the last part
/// closes the paste; callers must reserve the complete group before sending it.
/// Sanitizing and bracket mode remain the supervisor's responsibility.
pub fn pasted_text(text: &str) -> Vec<GridInput> {
    let parts = text_parts(text, GRID_MAX_PASTE_PART_BYTES, true);
    let last = parts.len().saturating_sub(1);
    parts
        .into_iter()
        .enumerate()
        .map(|(index, data)| GridInput::Paste {
            data,
            last: index == last,
        })
        .collect()
}

/// UTF-8 payload and JSON-escaped envelope limits are independent. Paste alone
/// treats CRLF as one indivisible unit; IME commits keep their original splits.
fn text_parts(text: &str, byte_limit: usize, preserve_crlf: bool) -> Vec<String> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut encoded = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        let mut end = start + ch.len_utf8();
        if preserve_crlf && ch == '\r' && chars.peek().is_some_and(|(_, ch)| *ch == '\n') {
            chars.next();
            end += 1;
        }
        let unit = &text[start..end];
        let cost: usize = unit
            .chars()
            .map(|ch| match ch {
                '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' | '"' | '\\' => 2,
                ch if ch < '\u{20}' => 6,
                ch => ch.len_utf8(),
            })
            .sum();
        // Leave the same conservative envelope allowance as grid-input.js.
        if !part.is_empty()
            && (part.len() + unit.len() > byte_limit
                || encoded + cost > GRID_MAX_INPUT_MESSAGE - 64)
        {
            parts.push(std::mem::take(&mut part));
            encoded = 0;
        }
        part.push_str(unit);
        encoded += cost;
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

#[cfg(test)]
mod tests;
