//! Which characters are unsafe to show verbatim when text from another host
//! or an agent reaches a human, and the one way the GUI spells them.
//!
//! Session titles, host identities, conversation ids, and refusal messages all
//! arrive from peers Farhelm does not control, and they are printed in CLI
//! tables, written to audit and hook logs, and drawn in the desktop app. Every
//! one of those surfaces has the same failure: a character that changes what
//! the reader SEES without looking like anything. This module is the single
//! answer to "which characters", so a spoofing defense that one surface
//! tightens is tightened everywhere.
//!
//! The notation is shared too where the readers are the same: everything the
//! desktop app shows (labels, link disclosures, startup alerts) writes an
//! escaped character as `<U+XXXX>` through [`notation`] and [`escaped`]. Other
//! surfaces keep a notation their own readers expect: the command line writes
//! `\u{202e}` and `\x1b` inside fields whose backslashes it escapes first, so an
//! agent parsing `farhelm agent sessions` reads one unambiguous form, and logs
//! replace with `_`.

/// Whether `ch` must never be shown as itself in peer-supplied text.
///
/// Three families, each with a known presentation attack:
///
/// - Unicode's `Cc` controls ([`char::is_control`]): the C0 range, DEL, and
///   C1. A newline forges an extra log line or table row; the rest can drive a
///   terminal.
/// - Characters that break or reorder a line without being controls: the line
///   and paragraph separators U+2028 and U+2029 (many viewers break lines on
///   them), and the bidirectional formatting characters: the marks U+200E,
///   U+200F and U+061C, the embeddings and overrides U+202A–U+202E, and the
///   isolates U+2066–U+2069. An override inside a value reverses the text that
///   follows it, so "recorded X, reported Y" can be made to read with the two
///   swapped while the bytes are unchanged.
/// - Invisible characters that let two different values render identically:
///   the soft hyphen U+00AD, the combining grapheme joiner U+034F, the Hangul
///   fillers U+115F, U+1160, U+3164 and U+FFA0, the Khmer inherent vowels
///   U+17B4–U+17B5, the Mongolian vowel separator U+180E, the zero-width
///   space, non-joiner and joiner U+200B–U+200D, the word joiner and
///   invisible operators U+2060–U+2064, the deprecated format characters
///   U+206A–U+206F, the zero-width no-break space (byte-order mark) U+FEFF,
///   the interlinear annotation controls U+FFF9–U+FFFB, and the tag block
///   U+E0000–U+E007F.
///
/// Variation selectors (U+FE00–U+FE0F, U+E0100–U+E01EF) are deliberately NOT
/// here, although they render as nothing: ordinary emoji carry them ("❤️"
/// is U+2764 U+FE0F), and escaping them would mangle every session title
/// that uses one. The tag block is escaped anyway, although subdivision flag
/// emoji use it, because those are rare in titles and tags are otherwise
/// pure hidden payload. Where two values must never look alike whatever they
/// contain (host identities, which are UUIDs), the surface escapes all
/// non-ASCII itself rather than relying on this list; see the desktop app's
/// `local_host::display_identity`.
///
/// Unicode's whole format category (`Cf`) is in, spelled out by code point as
/// of Unicode 17: besides the families above, the Arabic and Syriac
/// prepended concatenation marks (U+0600–U+0605, U+06DD, U+070F, U+0890,
/// U+0891, U+08E2, U+110BD, U+110CD), the Egyptian hieroglyph format
/// controls U+13430–U+1343F, the shorthand format controls U+1BCA0–U+1BCA3
/// and the musical symbol format controls U+1D173–U+1D17A. The old browser
/// GUI's terminal-link disclosure escaped the entire category through a
/// regular expression (its policy is pinned in the grid model's link oracle);
/// listing it here lets that disclosure use this table without showing less
/// than it did.
///
/// This is a KNOWN-BAD list, not a Unicode general-category test: the standard
/// library has no category tables, and escaping everything non-ASCII would
/// mangle every legitimately non-English title. A format character added by a
/// later Unicode version renders as itself until it is listed, which is why
/// the desktop app also draws host text in its own direction-isolated element
/// (SPEC_impl.md, "Host-supplied text").
pub fn is_presentation_unsafe(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061C}'
                | '\u{06DD}'
                | '\u{070F}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08E2}'
                | '\u{115F}'
                | '\u{1160}'
                | '\u{17B4}'..='\u{17B5}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206F}'
                | '\u{3164}'
                | '\u{FEFF}'
                | '\u{FFA0}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{110BD}'
                | '\u{110CD}'
                | '\u{13430}'..='\u{1343F}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

/// How the desktop app writes one escaped character: `<U+202E>`, at least four
/// upper-case hex digits. The one spelling every GUI surface uses, so a reader
/// learns it once.
pub fn notation(ch: char) -> String {
    format!("<U+{:04X}>", ch as u32)
}

/// `text` with every presentation-unsafe character written in [`notation`],
/// and every literal `<U+` that a peer wrote escaped as well (its `<` becomes
/// `<U+003C>`), so text can never pose as an escape this function produced.
/// Everything else, spaces included, is kept as it is.
///
/// The literal-notation rule is SPEC.md's for terminal-link disclosures ("make
/// presentation-unsafe characters visible, distinguish literal escape notation
/// from escaped characters"); the desktop app applies it to all host-supplied
/// text it shows, which is never weaker.
pub fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, ch) in text.char_indices() {
        if is_presentation_unsafe(ch) || (ch == '<' && text[index..].starts_with("<U+")) {
            out.push_str(&notation(ch));
        } else {
            out.push(ch);
        }
    }
    out
}

/// Quote one word so a POSIX-family shell reads it back byte for byte, with
/// no expansion of any kind.
///
/// Always single quotes, even for words that look plain. `shell_words::quote`
/// leaves words it considers safe bare, and its safe set includes characters
/// that are live syntax in the shells Farhelm actually hands command lines
/// to: braces (bash and zsh brace expansion), `!` (history expansion in the
/// interactive login shell an agent launches under), and `^`. Paths come from
/// `$HOME`, state-directory flags, and remote install locations, so a
/// directory named `a{1,2}` must not turn into two words. An embedded single
/// quote is closed, escaped, and reopened (`'\''`), the one encoding every
/// POSIX shell reads the same way.
///
/// Only for text that reaches a shell. Farhelm's own shell-word parsing
/// (`shell_words::split` on an invocation) round-trips either encoding.
pub fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{escaped, is_presentation_unsafe, notation};

    /// Why this matters: four surfaces used to keep their own copies of this
    /// set and had drifted apart, so a zero-width character escaped in the
    /// browser printed raw in `farhelm agent sessions`. This table is now the
    /// one place the set is stated.
    ///
    /// Specification: every listed character is unsafe, and ordinary text
    /// (ASCII, non-English letters, a plain space, emoji, and characters just
    /// outside each listed range) is not.
    #[farhelm_testtrace::test]
    fn the_unsafe_set_is_exactly_the_documented_families() {
        let unsafe_chars = [
            '\n',
            '\r',
            '\t',
            '\0',
            '\u{1b}',
            '\u{7f}',
            '\u{85}',
            '\u{9f}',
            '\u{00AD}',
            '\u{061C}',
            '\u{180E}',
            '\u{200B}',
            '\u{200C}',
            '\u{200D}',
            '\u{200E}',
            '\u{200F}',
            '\u{2028}',
            '\u{2029}',
            '\u{202A}',
            '\u{202B}',
            '\u{202C}',
            '\u{202D}',
            '\u{202E}',
            '\u{2060}',
            '\u{2061}',
            '\u{2062}',
            '\u{2063}',
            '\u{2064}',
            '\u{2066}',
            '\u{2067}',
            '\u{2068}',
            '\u{2069}',
            '\u{FEFF}',
            '\u{034F}',
            '\u{115F}',
            '\u{1160}',
            '\u{17B4}',
            '\u{17B5}',
            '\u{206A}',
            '\u{206F}',
            '\u{3164}',
            '\u{FFA0}',
            '\u{FFF9}',
            '\u{FFFB}',
            '\u{0600}',
            '\u{0605}',
            '\u{06DD}',
            '\u{070F}',
            '\u{0890}',
            '\u{0891}',
            '\u{08E2}',
            '\u{110BD}',
            '\u{110CD}',
            '\u{13430}',
            '\u{1343F}',
            '\u{1BCA0}',
            '\u{1BCA3}',
            '\u{1D173}',
            '\u{1D17A}',
            '\u{E0000}',
            '\u{E0041}',
            '\u{E007F}',
        ];
        for ch in unsafe_chars {
            assert!(
                is_presentation_unsafe(ch),
                "U+{:04X} must be escaped",
                ch as u32
            );
        }
        let safe_chars = [
            'a',
            ' ',
            '~',
            'é',
            'ß',
            'Ж',
            'ع',
            '日',
            '😀',
            '\u{00AC}',
            '\u{00AE}',
            '\u{200A}',
            '\u{2010}',
            '\u{2027}',
            '\u{202F}',
            '\u{2065}',
            '\u{FEFE}',
            '\u{034E}',
            '\u{17B3}',
            '\u{17B6}',
            '\u{2070}',
            '\u{3165}',
            '\u{FFF8}',
            '\u{FFFC}',
            '\u{E0080}',
            // Just outside the format characters added with the whole category.
            '\u{0606}',
            '\u{06DE}',
            '\u{0710}',
            '\u{08E3}',
            '\u{1342F}',
            '\u{13440}',
            '\u{1BC9F}',
            '\u{1BCA4}',
            '\u{1D172}',
            '\u{1D17B}',
            // Variation selectors stay unescaped so emoji keep rendering.
            '\u{FE0F}',
            '\u{FE00}',
            '\u{E0100}',
        ];
        for ch in safe_chars {
            assert!(
                !is_presentation_unsafe(ch),
                "U+{:04X} is ordinary text and must be shown as itself",
                ch as u32
            );
        }
    }

    /// Why this matters: the desktop app shows host-supplied text in several
    /// places (labels, link disclosures, startup alerts) that used to spell
    /// escapes on their own; one function means one spelling and one rule for
    /// literal notation a host might type to impersonate an escape.
    ///
    /// Specification: unsafe characters become `<U+XXXX>` with at least four
    /// upper-case hex digits, a literal `<U+` opener has its `<` escaped so it
    /// can never read as an escape, a `<` that does not open `U+` is kept, and
    /// ordinary text (spaces, letters, emoji with variation selectors) is kept
    /// byte for byte.
    #[farhelm_testtrace::test]
    fn escaped_text_spells_every_unsafe_character_and_literal_notation_visibly() {
        assert_eq!(notation('\u{202E}'), "<U+202E>");
        assert_eq!(notation('\u{1D173}'), "<U+1D173>");
        assert_eq!(notation('\u{7}'), "<U+0007>");
        assert_eq!(escaped("v1\u{202E}2"), "v1<U+202E>2");
        assert_eq!(escaped("a\nb"), "a<U+000A>b");
        assert_eq!(escaped("<U+202E> fake"), "<U+003C>U+202E> fake");
        assert_eq!(escaped("a < b <U"), "a < b <U");
        assert_eq!(escaped("  two  spaces  "), "  two  spaces  ");
        assert_eq!(
            escaped("caf\u{e9} \u{2764}\u{FE0F} \u{65e5}"),
            "caf\u{e9} \u{2764}\u{FE0F} \u{65e5}"
        );
    }

    /// Why this matters: Farhelm hands command lines to the user's login
    /// shell (agent launches run under `$SHELL -l -i -c`) and to remote
    /// shells over ssh, with paths it did not choose inside them. A word the
    /// shell expands (braces, history, variables, globs) launches the wrong
    /// program or splits a path in two.
    ///
    /// Specification: each hostile word, quoted and handed to `sh -c` (and to
    /// `bash -c` when bash is installed), prints back exactly as given, and
    /// the quoted form still splits back to the word through the same
    /// shell-word parser Farhelm uses for invocations.
    #[cfg(unix)]
    #[farhelm_testtrace::test]
    fn shell_quote_round_trips_hostile_words_through_real_shells() {
        use super::shell_quote;
        let words = [
            "plain",
            "",
            "with space",
            "brace{a,b}",
            "bang!history",
            "caret^x",
            "dollar$HOME",
            "back`tick`",
            "glob*?[x]",
            "single'quote",
            "double\"quote",
            "back\\slash",
            "tilde~/x",
            "semi;colon&and|pipe",
            "new\nline",
        ];
        let shells: Vec<&str> = ["sh", "bash"]
            .into_iter()
            .filter(|shell| {
                std::process::Command::new(shell)
                    .args(["-c", "true"])
                    .status()
                    .is_ok_and(|status| status.success())
            })
            .collect();
        assert!(
            shells.contains(&"sh"),
            "a POSIX sh is required for this test"
        );
        for word in words {
            let quoted = shell_quote(word);
            assert_eq!(
                shell_words::split(&quoted).unwrap(),
                vec![word.to_string()],
                "{quoted}"
            );
            for shell in &shells {
                let out = std::process::Command::new(shell)
                    .args(["-c", &format!("printf %s {quoted}")])
                    .output()
                    .unwrap();
                assert!(out.status.success(), "{shell} failed on {quoted}");
                assert_eq!(
                    String::from_utf8(out.stdout).unwrap(),
                    word,
                    "{shell} did not read {quoted} back verbatim"
                );
            }
        }
    }
}
