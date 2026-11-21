# Save and Close can silently abandon Tensorlake and ntfy drafts while a test runs

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can give up without explanation while a Notifications or Tensorlake test is running.

## Details

The ordinary footer disables Save on holds_save (settings/editing.rs:336), but ask_to_close only identifies
EditState::Saving as pending (:567–595), and the resulting batch dispatches the form directly (:342–374, :404–415).
Tensorlake Test stays available with a saved key even when a replacement is dirty (settings/tensorlake_key.rs:577–594)
and sets working while it checks the stored key (:511–538). The replacement remains Dirty (:394–407) and Save returns
false without an error while working (:373–375, :417–421). ntfy Send test also stays available with stored configuration
alongside a dirty draft (settings/ntfy.rs:725–739, :804), and its Save has the same silent return when testing
(:366–368, :453–467, :481–483, :534–559). Settings drops the batch and selects the page (editing.rs:441–450, :525–543).
Preserve the Save and Close decision until the test finishes, or explain the busy refusal and leave the answer
available. SPEC.md:2454–2457 requires either sequencing the saves or showing the affected form with its failure.
SaveReceipt's boolean cannot distinguish this non-admission from a submitted save's refusal (editing.rs:64–88, :404–415,
:438–450). A real refusal is form-owned, whereas no rejection text is produced in the Test-lock path. This is
independent of l3's hidden recorded refusal and l10's missing operation receipt.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footer/l5`, `footerTests/l6`, `footerEditors/f2`.
