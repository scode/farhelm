# Undo cannot restore legacy templates rejected by the new session-name rule

Reviewed commit: e58aafe5a8afe97d7bc7672fe4519eddcf4044e2

## TLDR

Deleting an older template containing joined emoji or hidden characters offers Undo, but Undo always fails and the
retained copy expires.

## Details

Suggested bucket: `highest`. Confidence: confirmed by source inspection; no runtime reproduction. Source: dedicated
`gpt-6-astra` high adversarial review. Proposed drop: none; any proposed acceptance under the save-time restriction
requires independent highest-candidate review.

The new validation at crates/farhelm-proto/src/launcher.rs:347–362 rejects previously valid session names, including
joined emoji. Existing templates remain loadable and deletable. crates/farhelm-ui/src/list/templates.rs:1029–1059
deletes the stored template and retains its only UI recovery copy for ten seconds; :1081–1092 restores through ordinary
PUT. That reaches crates/farhelm-helm/src/templates.rs:85–91 and deterministically fails the new validation. The UI
nevertheless promises Undo at templates.rs:1211 and :1232. Trigger: delete a template saved before this change with, for
example, a joined-emoji session name, then immediately click Undo. Consequence: restoration fails even with a free name
and healthy server; the recovery copy subsequently disappears. Preserve a repair/recovery path or disclose the
irreversible case before deletion without weakening ordinary save validation.

Possible cover: SPEC.md:220–223 intentionally rejects saving legacy unsafe values, but does not explicitly withdraw
Delete's Undo promise at :618–619. SPEC_impl.md:4178–4180 promises restoration subject to name availability.
TRIAGE_OUTCOMES.md heading template-supplied-session-names-rendered-raw.md authorizes validation on every save path;
whether that intentionally includes unrecoverable deletion deserves explicit judgment. The saved-snapshot queue item
concerns different timing and content loss.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. The validator and helm validation path
are unchanged; current templates.rs:1097–1130 still deletes and expires the recovery copy, and :1152–1163 still restores
through PUT. No fixing commit found.

Review coverage: full commit diff read: true. No part of the reviewed commit diff remained unread. Unchanged surrounding
modules, most unrelated specification/ledger sections, and unrelated queue finding bodies were not exhaustively read.
Some aggregate outputs truncated search/context material; relevant missing material was reopened in smaller reads. The
unrelated remainder of the later settings-dialog diff was not pursued. No Sol reports or working-copy queue additions
were read.

Runtime evidence: None, as requested by default. Applied the supplied test-authoring checklist to every changed
test/fixture by inspection. Findings are source-confirmed, without browser reproduction. No checkout files were created,
modified or deleted; no fetch, rebase or nested agents.

## Additional affected scope

Reviewed commit: dd9573cc07782949917597520bdc49ca84706cc0

## TLDR

Deleting an otherwise usable legacy template can permanently lose it even when the user immediately clicks Undo.

## Details

Suggested bucket: `highest`. Confidence: confirmed by source inspection; no runtime reproduction. Source: dedicated
`gpt-6-astra` high adversarial review. Proposed drop: none.

The new rejection in crates/farhelm-proto/src/launcher.rs:336–353 also applies to restoration. In
crates/farhelm-ui/src/list/templates.rs:1022–1052, Delete removes the stored template and retains its original fields
only in a ten-second Undo value. Undo sends those unchanged fields through put_template at :1074–1085;
crates/farhelm-helm/src/templates.rs:85–91 rejects their legacy controls or invisible characters. The error leaves the
template deleted, and the timer discards the remaining copy. Trigger: delete a pre-rule template containing, for
example, a quoted literal newline in either command, then Undo before expiration with its name still free. This newly
breaks the restoration contract in SPEC.md:601–603. Preserve a supported restoration path or prevent this irreversible
deletion without explaining the limitation.

Possible cover: SPEC.md:219–222 and TRIAGE_OUTCOMES.md headings
template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md and its resume counterpart require
rejection on every save. They do not explicitly withdraw Delete's Undo promise or accept losing the original template
during restoration. This contract interaction needs triage.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: template deletion/expiration at UI
templates.rs:1097–1130, restoration through ordinary PUT at :1152–1163, shared rejection at helm templates.rs:85–91 and
proto launcher.rs:347–363. No fixing commit found.

Review coverage: full commit diff read: true. No commit-diff content remains unread. One aggregate tool output was
truncated; the affected diff section was reopened, and relevant specification passages were subsequently read directly.
Full surrounding modules, bundled xterm internals, complete SPEC documents and unrelated ledger entries were not
exhaustively read. No Sol report or new working-copy queue file contents were read; an initial git-status listing
incidentally exposed working-copy filenames. No checkout files were created, changed or deleted; no nested agents were
used.

Runtime evidence: None run, as requested by default. Findings rely on historical source, caller/lifecycle tracing,
original triage records and pinned-main comparison. Browser normalization and practical visual deception were not
reproduced; no test pass is claimed.
