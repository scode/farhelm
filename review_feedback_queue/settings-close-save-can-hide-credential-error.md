# Settings can hide the credential error that stopped Save and Close

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Settings can keep a failed credential save’s explanation behind the Credentials overview.

## Details

Two different unsaved Tensorlake keys invoke reveal_key_conflict and show_credential_key (settings/editing.rs:390–394,
:519–523), which put the Credentials overview in front while retaining the open credential draft
(settings/credentials.rs:291–307). The overview lets the user resolve the key conflict through the footer without
leaving that state. The next Save and Close still includes the hidden credential (settings/editing.rs:255–258). On
failure, editing.rs:441–450 calls reveal_edit_failure, but :526–543 only selects Credentials for a credential target and
leaves key_revealed set. The overview at credentials.rs:580–603 offers Resume editing credential without the error, and
:672–677 unmounts the editor. The refusal is retained by credentials/editor.rs:321. SPEC.md:2456–2457 and :2479–2480
require showing the failed form and reason. Reveal the exact failed credential editor before returning to the section,
as the key failure branch already does for its form. The coordinator has the exact failed entity in edit_finished
(editing.rs:435–450), but failure recovery reduces it to a section except for a Credentials key (:525–543). This is a
presentation failure after a real refusal. It is independent of l5's absence of a save reason, nav/l5's rendered refusal
below the viewport, and footerEditors/l8's committed replacement with delivery warnings.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l3`, `nav/l8`, `footerEditors/f1`.
