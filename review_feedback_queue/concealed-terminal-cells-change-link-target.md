# A printed terminal link can open a hidden destination

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A printed link can look like one site but open another when the terminal hides part of the address. Farhelm’s ordinary
printed-link click does not disclose that difference.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R5 Opus 1.

Source references: `crates/gui/farhelm-gui/src/terminal/attached.rs:2680–2750, 3150–3156`,
`crates/gui/farhelm-gui/src/terminal.rs:1185–1225`, `crates/gui/farhelm-grid-model/src/links.rs:262–384`.

Grid link detection reads the underlying characters, including concealed runs or text rendered in the same color as its
background. Painting can leave that part unreadable. The resulting Text link is admitted without the destination-display
check used for an OSC 8 hyperlink, so a hidden hostname extension can change the browser origin.

Opus rated the source route confirmed and noncritical for the migration, as phishing-class behavior carried over from
the older app. There is no separate verification verdict. Highest reflects a shipped route to opening a host-supplied
destination other than the one disclosed. SPEC.md lines 3145–3149 exempts ordinary printed URLs from disclosure because
what is shown is what opens. The OSC 8 warning's documented limitations concern a different path.

Require exact destination disclosure for a printed link with concealed cells, or refuse links across them. Verify both
concealment and matching foreground/background without weakening ordinary visible links.
