# A template-list pick can launch a replacement under the same name

Reviewed commit: 51f07fa7abfc5dfcd3020a304d8740e5cbe44c16

## TLDR

If a template is deleted and recreated under the same name while the template list is open, selecting its old row can
directly launch the replacement. The displayed summary can still describe the previous host, command or agent choices.

## Details

The Cmd-J list keeps a template snapshot but hands off only the selected name
(`crates/farhelm-ui/src/list/quick_switcher.rs:391–413`). The shared worker looks up that name in the current catalog
(`crates/farhelm-ui/src/launch_sequence.rs:285–295`; `crates/farhelm-ui/src/list/view.rs:3667–3674`). An absent name
becomes a preparation error (`view.rs:3577–3589`); a replacement with the same name resolves successfully and can launch
directly. No identity or definition comparison distinguishes it from the row the user saw.

To verify, open Cmd-J with a runnable template, delete it and recreate a runnable template with the same name through an
approved agent operation, changing a visible launch choice, then select the old row. Compare the displayed summary with
the submitted launch. No such runtime sequence or harmful launch was performed during review.

Suggested bucket: **highest**, conservatively, for possible unintended execution on another host or with different
command or permission choices. Confidence: **confirmed name-only resolution mechanism**; harmful execution and the full
concurrent sequence are unverified. Provenance: same-model adversarial trailing review and an independent fresh-context
exclusion audit. Proposed drop: none after audit.

Possible cover: SPEC.md's Cmd-J paragraph and `SPEC_impl.md:994–1000` explicitly require a fresh name lookup. Ordinary
same-name edits may therefore be intended to affect this launch. However, the Cmd-J paragraph also promises a
preparation error for a template deleted or renamed since opening. Agent delete/recreate is supported, including for
changing template kind, and the agent-authority rules do not waive GUI correctness. The independent auditor could not
unambiguously exclude the delete/recreate remainder. The existing `queued-launch-can-change-template.md` and its ledger
decision concern armed shortcut references changing bindings; they do not cover this direct row pick. No matching
Planned item, known-bug acceptance or filter was found. Triage should reconcile the fresh-lookup and deletion promises
before commissioning an identity mechanism.
