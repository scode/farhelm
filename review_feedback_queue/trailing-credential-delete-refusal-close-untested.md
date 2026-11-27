# A credential Delete refused while a close of Settings waits on it is untested, though not dropping that refusal is why the wait was added

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Closing Settings while a credential Delete is still out now waits for the helm's answer, so that a refused Delete is not
closed over unseen. The only test of this checks the case where the Delete succeeds and Settings closes. Nothing checks
that a refused Delete keeps Settings open on the credential's editor with the helm's reason, so a regression there would
let Settings close and hide the refusal.

## Details

a_close_waits_for_a_credential_delete (credential_tests.rs:211-236 at the tip; added in ee8ef7294a) gives its reason in
its own docstring: "a credential Delete has no draft for the close question to name, so a close that went on over it
would drop a refusal unseen". It then pins only the Done reply: Escape waits, and the done answer closes the editor and
Settings. The refusal route at the tip is: the Delete completion queues save_receipt.settled_other(true)
(editor.rs:377-381), then resets deleting and records the error (editor.rs:388-393). The armed close receipt from
arm_close (editing.rs:1015-1040) calls close_settled(Some(CloseRefusal::edit)) (editing.rs:1054-1084). That finds the
editor in loaded() again, runs reveal_edit_failure, which shows Credentials and brings the editor forward through
Credentials::reveal_editor (editing.rs:808-819), and does not close. Source reading suggests this works today, but no
test covers it. Any regression would close Settings over the refusal: the settled_other call dropped or moved after the
editor goes, the refused flag inverted, or close_settled's refusal branch bypassed.
delete_is_unavailable_while_a_recipe_uses_the_credential (credential_tests.rs:806-850) tests a refused Delete only
without a close waiting. How to verify: grep credential_tests.rs for tests that press credential-delete; none of them
combines an Escape or Close with a refused (4xx) reply. Fix: extend a_close_waits_for_a_credential_delete, or add a
sibling test, so the Delete is answered with a refusal while Escape waits. Then assert Settings stays open on
Credentials, the editor is drawn with credential-error carrying the helm's words, and no prompt is shown. This is the
credential counterpart of g03/f1, which covers the custom agent Delete's close-resume route; it is not a restatement of
it.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. This is a test-coverage gap with no known wrong behavior today.
- Confidence: confirmed (the coverage gap). The behavior of the untested route is inferred from source only.
- possible_cover: g03/f1 covers the analogous custom agent route, not this one.
- proposed_drop: none.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g01/f2 (finding), group g01 "Settings credential editor stays open until its save answers", findings log line 5.
