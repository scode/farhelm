# display_peer misses several invisible characters

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A remote host can make the "recorded" and "reported" identities in a host-adopt prompt look identical, so the user may
approve an install they would otherwise have noticed was different.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F30 / SEC-DISPLAY-PEER-MISSES-INVISIBLES`, tagged **possible**. Anchors and title: `crates/farhelm-ui/src/peer.rs:71` —
display_peer's escape list misses several invisible characters, so different peer values can render identically

`display_peer` decides what to escape with `must_escape` (line 71), a hand-written list of known-bad characters. It
covers C0/C1 controls, the bidi controls, U+00AD, U+200B–200D, U+2060, U+180E, U+FEFF and U+2028/2029. It does not cover
several other characters that also render as nothing:

- U+2061–U+2064 (invisible math operators);
- U+206A–U+206F (deprecated format characters);
- the tag block, U+E0000–U+E007F;
- variation selectors, U+FE00–U+FE0F and U+E0100–U+E01EF;
- U+034F (combining grapheme joiner);
- the Hangul fillers U+115F, U+1160, U+3164 and U+FFA0;
- U+17B4 and U+17B5;
- U+FFF9–U+FFFB.

There is also an edge case in the "nothing visible" fallback. `display_peer` counts anything that is not
`char::is_whitespace` as visible, and U+3164 is not whitespace. So a value consisting only of U+3164 renders as a blank
instead of "(whitespace only …)". The helm's own log escaper (`escape_for_log` in `agent_requests.rs`) already treats
U+2060–U+2064 as presentation-bending, so the two sides disagree. Direction isolation, the second defence layer, does
nothing against invisibility.

The concrete exposure is the hosts panel's adopt prompt, "recorded as install X; now reports Y". It exists so that the
user notices when a destination is a different Farhelm installation. A remote host can report an identity that differs
from the recorded one only by such characters. The two identities then render identically, and the user may approve what
looks like a false alarm. `peer.rs` states its purpose as ensuring that two different identities never render
identically and that none renders invisibly. The suggested fix is to add the default-ignorable ranges to `must_escape`
(ideally as one list shared with `escape_for_log`), to count only characters that actually draw a glyph as visible, and
to pin each range in a test. The reviewer's claim is based on Unicode properties and was not exercised in a browser.
