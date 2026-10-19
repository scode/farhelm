# A template list can advertise a key that launches another template

Reviewed commit: 51f07fa7abfc5dfcd3020a304d8740e5cbe44c16

## TLDR

After an agent moves a shortcut, the template list can still show the key beside its previous template. A user who
closes the list and uses that advertised key can launch the newly bound template instead.

## Details

Cmd-J builds its shortcut badges from cached `SharedPreferences`
(`crates/farhelm-ui/src/list/quick_switcher.rs:172–184,398–413`), while actual shortcut invocation reads fresh bindings
(`crates/farhelm-ui/src/list/view.rs:3506–3550`). The stale badge can remain until reload; it is not just a brief update
window. An approved agent shortcut move supplies a supported source of change without requiring another GUI window.

To verify, load preferences with key 1 bound to template A, have an agent move key 1 to runnable template B, then open
Cmd-J. Check whether A still advertises 1; close the list and invoke that key, comparing the launched template with the
badge. No mistaken gesture or harmful launch was reproduced. Refreshing labels or making the relationship between the
displayed assignment and invocation explicit could address the discrepancy, subject to the intended cache contract.

Suggested bucket: **highest**, conservatively, for the possible display-guided launch on an unintended host or with
unintended choices. Confidence: **confirmed stale-display and fresh-dispatch mechanisms**; the user's mistaken gesture
and resulting harm are unverified. Provenance: same-model adversarial trailing review and an independent fresh-context
exclusion audit. Proposed drop: none for this remainder.

Possible cover: `SPEC.md:1275–1282`, “Session list,” explicitly permits loaded clients to keep cached preference
behavior until reload and requires template shortcuts to read bindings on invocation. The independent auditor agreed
that this covers stale badge display alone. It could not unambiguously conclude that it accepts a badge guiding a later
launch to another target, especially alongside the Cmd-J paragraph's assigned-shortcut wording and `SPEC_impl.md:994`'s
current labels. This record retains only that uncertain consequence. The armed-sequence finding
`queued-launch-can-change-template.md` has a different trigger, and `stale-template-dialog-undoes-agent-shortcut.md`
concerns stale writes, not these badges. No exact Planned item, known-bug acceptance, ledger decision or filter covers
the remainder. Triage may decide the accepted cache contract is sufficient; this is not a reproduced security defect.
