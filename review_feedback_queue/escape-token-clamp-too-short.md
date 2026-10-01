# Shortening a host label can split an escape marker

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

For a host whose name contains certain invisible tag characters, the shortened accessible label in the host menu can end
in a broken `<U+E0041…` marker, which reads like text the host sent instead of Farhelm's escape.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F30 / COR-ESCAPE-CLAMP`, tagged **definite**. Anchor and title: `crates/farhelm-ui/src/menu_panel.rs:1183` —
shortening a host's label can still cut an escape token in half.

Text that comes from a remote host, such as its reported name, is untrusted. Before the UI shows it, `display_peer`
(`crates/farhelm-ui/src/peer.rs:73`) replaces every unsafe character (bidi overrides, control characters and similar)
with a visible token of the form `<U+XXXX>`. The host-actions menu's accessible label is built from that escaped name
and then shortened to 64 characters by `clamp_title`. That uses `back_off_from_split_escape` (`menu_panel.rs:1183-1187`)
so the cut never lands inside a token, because a half token like `<U+20` reads as literal text the host sent, which
defeats the escaping.

That helper assumes every token is exactly 8 characters (`<`, `U`, `+`, four hex digits, `>`) and so looks back only 7
characters for the opening `<`. But the unsafe set (`crates/farhelm-proto/src/text.rs`) includes the Unicode tag
characters U+E0000–U+E007F. `{:04X}` prints those with five hex digits, giving 9-character tokens like `<U+E0041>`. If
the 64-character cut falls just before such a token's closing `>`, the 7-character look-back does not reach the `<`, the
cut stays where it is, and the label ends in a broken `<U+E0041…`. The doc comment's claim that a token is "always
exactly eight ASCII bytes" is false.

Low severity: it affects one cut position in one accessible label, but it is exactly the failure the function exists to
prevent. Suggested change: size the look-back for the longest possible token (`<U+10FFFF>`, 10 characters, so look back
9), or scan back for the nearest `<` within that bound. Fix the doc comment, and add a test with the 64-character
boundary inside a `<U+E00xx>` token.
