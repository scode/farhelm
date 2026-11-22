# A typed recent match says Enter replaces the session even though it only fills the form

Reviewed commit: 868c0cea4bf58a4e8b7b67d8f301f4083488de66

## TLDR

Enter on a typed recent result only fills the form even though its key cue says Replace.

## Details

**other.** Source inspection.

868c0cea4bf58a4e8b7b67d8f301f4083488de66:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:513–558;
1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:522–565;
crates/gui/farhelm-gui/src/launcher/view/search.rs:246–270,277–300,343–359,379–406,513–517,933–942

In a wide Replace with launcher, a highlighted typed recent result announces Enter as Replace even though its Enter only
fills. LM-02 and LMC-01 describe the same visual and accessible cue introduced by #4461. An ordinary recent is enabled:
Replace holds Host, while the recent sets Destination and an ordinary recent has no new environment target. The selected
inline row passes the Replace cap and the typed handler only calls take_recent. Narrow typed results take a different
renderer; empty highlighted Enter is the separate direct submit; the empty option's own accessible label is a separate
surface (replace-option-accessible-action and LMC-09). SPEC.md:1386–1389,1433–1436 at the reviewed tip distinguishes the
actions. The reviewer suggested other for misleading wording, no dispatch or deletion on this route; confidence
source-confirmed; ; Possible coverage: LM-02 and LMC-01 exact.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/typed-recent-false-replace-cue`, with `launcher-matches/LM-02`,
`launcher-matching-contract/LMC-01`, `launcher-search/LS-05`.
