//! Tests of Farhelm's patched `vte` and `alacritty_terminal` working together.
//!
//! Each vendored crate's own tests exercise it alone, and `alacritty_terminal`
//! built alone uses crates.io's `vte`, so a behavior that one patch produces
//! through the other shows up in neither suite. This crate depends on the
//! vendored `alacritty_terminal` and swaps in the vendored `vte` the way
//! Farhelm's workspace does (see `Cargo.toml`). It holds nothing but tests;
//! vendor/README.md has the command that runs them.

#[cfg(test)]
mod tests {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::{OSC_PAYLOAD_LIMIT, ansi};

    /// Spec: while a hyperlink is active, an OSC 8 that is too long ends it,
    /// and the text after it is unlinked, whichever layer refuses the OSC 8:
    /// the terminal (URI or `id` over its own limits, payload at most
    /// `OSC_PAYLOAD_LIMIT`, which the parser dispatches, as vte's
    /// `osc_at_payload_limit_is_dispatched` shows) or the parser (payload one
    /// byte over `OSC_PAYLOAD_LIMIT`, so it is dropped and only reported).
    /// Both the URI and the `id` are tested on each side of the parser's
    /// limit.
    ///
    /// Why: the parser used to drop an oversized OSC 8 without telling the
    /// terminal, so a rejected link sent long enough left the previous link
    /// active, and the text after it was attributed to a URI it never had.
    /// Only this pairing can show that, since each crate's own tests use
    /// one patch at a time. That this crate compiles at all shows the `vte`
    /// is the patched one: crates.io's has no `OSC_PAYLOAD_LIMIT`.
    #[test]
    fn too_long_hyperlink_ends_active_link_at_either_layer() {
        // The parser counts every OSC byte but the `;` separators and the
        // terminator, so `8;;<uri>` buffers 1 + URI bytes, and
        // `8;id=<id>;http://e/` buffers 1 + 3 + id + 9 bytes.
        let uri = |len: usize| format!("\x1b]8;;{}\x07", "u".repeat(len));
        let id = |len: usize| format!("\x1b]8;id={};http://e/\x07", "i".repeat(len));
        let cases = [
            ("URI at the parser's limit", uri(OSC_PAYLOAD_LIMIT - 1)),
            ("URI over the parser's limit", uri(OSC_PAYLOAD_LIMIT)),
            ("id at the parser's limit", id(OSC_PAYLOAD_LIMIT - 13)),
            ("id over the parser's limit", id(OSC_PAYLOAD_LIMIT - 12)),
        ];

        for (case, osc8) in cases {
            let mut term = Term::new(Config::default(), &TermSize::new(10, 2), VoidListener);
            let mut parser: ansi::Processor = ansi::Processor::new();
            parser.advance(&mut term, b"\x1b]8;;http://previous/\x07a");
            parser.advance(&mut term, osc8.as_bytes());
            parser.advance(&mut term, b"b");

            let first = term.grid()[Line(0)][Column(0)].hyperlink();
            assert_eq!(
                first.as_ref().map(|link| link.uri()),
                Some("http://previous/"),
                "{case}"
            );
            let second = &term.grid()[Line(0)][Column(1)];
            assert_eq!(second.c, 'b', "{case}");
            assert_eq!(second.hyperlink(), None, "{case}");
        }
    }
}
