# A failed close-save can open Notifications with the refusal below the viewport

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can take the user to Notifications while leaving the relevant error below the viewport.

## Details

`crates/gui/farhelm-gui/src/settings/editing.rs:437–449, 525–543` switches to the failing section without revealing the
failed form or its outcome. A section switch sets the page offset to zero (`settings.rs:870–875`). Notifications puts
the four Sounds rows above ntfy (`settings.rs:1329–1354, 1504–1506`), and the ntfy refusal is in the final card row
after the form and Send test (`settings/ntfy.rs:753–820, 830–854`). At the documented 560px minimum height, the 5%
margins, pinned header, heading and footer leave under 335px for the page even before counting the footer button
(`components/dialog.rs:64–65, 356–370, 408–418, 588–598, 700–724`; `theme.rs:525–527`;
`settings/frame.rs:369–372, 413–422`). The four padded sound rows plus the ntfy heading, switches and two 36px input
rows already put the refusal beyond that space (`settings/card.rs:25–38, 112–127`; `settings/ntfy.rs:566–590, 753–820`).
The existing close-failure test checks the presence of an accessibility label and does not compare it against the
viewport (`settings/tests.rs:3258–3294, 2595–2601`). SPEC.md:2454–2457 calls for showing the page with its error. Reveal
the failed form or give its refusal a visible location above the pinned actions.

Confidence: confirmed by inspection of the section order and layout bounds.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `nav/l5`.
