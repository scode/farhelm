# Approval cards can obscure a file's destination before transfer

Reviewed commit: afc828776596f2e6639d16d4203ccc8b3d0f5862

## TLDR

A terminal file link can remain clickable while an approval card hides the hover details needed to judge its
destination. This can potentially admit a download, or modifier-opening an eligible downloaded file, without a visible
disclosure.

## Details

Suggested bucket: conservatively `highest`, because admission depends on showing the user the file destination.
Confidence: likely; the source contains the overlapping layers and geometry-only admission, but browser/native geometry,
event delivery and harmful transfer were not reproduced. Provenance: Astra high repeat review's independent assessment
of an archived security lead. Proposed drop: none.

`crates/farhelm-ui/assets/terminal-files.js:12–18` checks disclosure dimensions, overflow and viewport containment, not
occlusion. Lines 247–248 place the disclosure beside the pointer. A pointer outside an approval card can therefore put
part of its disclosure beneath that card. Lines 419–445 admit the transfer after this geometry check and use the
previously disclosed canonical path. The disclosure has layer 40 (`crates/farhelm-ui/assets/app.css:1236–1249`), while
opaque approval cards have layer 45 (`:7023–7041`).

`crates/farhelm-ui/assets/approval-layout.js:19–31` positions those cards over part of the terminal, and
`crates/farhelm-ui/src/approvals.rs:143–161` leaves the uncovered terminal usable. A click on an uncovered file link can
therefore reach the terminal even though its destination disclosure is partially obscured. Viewport containment alone
does not establish that the whole disclosure is visible to the user.

Keep transfer admission conditional on a fully visible disclosure when approval cards are present. A targeted browser
regression should place the pointer outside a card with the hover overlapping it, then distinguish refused transfer from
ordinary full-disclosure admission. The native modifier-opening route deserves the same admission check.

Possible cover: none established. SPEC.md:1674–1693 and 2753–2761 tie file admission to the hover's target disclosure.
The existing oversized-disclosure fix concerns fitting within the viewport; the queued zero-height approval-region issue
has a different consequence. The scrollable-dialog filter does not cover a pointer-transparent disclosure hidden behind
another surface. No exact Planned, BUGS, queue or ledger coverage was found.

The lead came from `plans/reports/terminal-link-safety.report.md:100–102` at the parent of deleting commit
7a40e09b1bcddf5bf1e48681d9e1d7d50337df58; the report explicitly called it suspected. The repeat review traced the
current source rather than treating that report as proof. This is a pre-existing adjacent issue, not a claim that the
approval-record deletion introduced it. No runtime or historical private evidence was used.

A further fresh Astra high reviewer confirmed the stacking and reachable-click premises: the disclosure is appended to
the terminal at `terminal-files.js:211–248`, while approval cards are shell siblings
(`crates/farhelm-ui/src/lib.rs:1668–1696`). The inspected ancestors create no stacking context reversing their order. A
pointer just left of the centered card can click an uncovered link while its right-offset disclosure overlaps the card.
The ordinary gesture gate (`terminal-links.js:110–129`; `terminal.js:5493–5504`) and OSC file adapter
(`terminal.js:3647–3664`) preserve that click path. Confidence remains likely because rendering and transfer were not
exercised. This establishes neither arbitrary execution nor secret exfiltration.
