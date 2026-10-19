# Reapplying an unchanged template can duplicate a launch

Reviewed commit: e4207f233dbb6fefcbe52e0ee01fb7ef5ee6331b

## TLDR

After a quick launch succeeds but its reply is lost, pressing the same template shortcut in the revealed launcher and
then Launch can start a second session even when no launch choice changed.

## Details

The new shortcut replay uses ordinary template acceptance (`crates/farhelm-ui/src/list/create_form.rs:4575–4580`). Every
accepted template clears `intent_key` unconditionally (`840–843`). Ordinary existing-directory creation reuses a
retained key only while it remains present (`5327–5344`), and the transport-error path deliberately preserves it
(`5649–5657`). Reapplying the same effective template can thus discard the identity that would have let the supervisor
recognize the retry.

Reproduce with an existing-directory command template whose application changes no effective request field. Let its
initial quick-launch request complete on the helm while dropping the response. In the revealed launcher, press the same
template shortcut and then Launch; compare request keys and session count. Managed-checkout mode has additional
retained-attempt reconciliation and is not the minimal reproduction. Template application should preserve the retained
identity when the effective request binding stays the same.

Suggested bucket: **high**, for a duplicate launch. Confidence: **confirmed static control flow**, not runtime
reproduced. Harm from concurrent agents modifying the same work is unverified; no highest-priority consequence is
established. Provenance: same-model adversarial trailing review of the stated commit. Proposed drop: none.

Possible cover: `TRIAGE_OUTCOMES.md`, `retrying-through-unchanged-permission-choice-launch-duplicate.md`, covers effort,
permission and trust controls specifically. `default-approval-reset-renews-retry-key.md` covers normalization after a
null-permission template, rather than reapplying an unchanged template. Neither covers this trigger. `SPEC.md:807–809`
requires one session per intended create. The landing report also acknowledges the template case in
`plans/reports/launcher-template-keys.report.md:120–125`. No matching Planned item, known-bug acceptance, queue item or
filter was found.
