# Uninstall path resolution strips embedded newlines, silently defeating the state-inside-lib identity guard

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

On a remote host whose directory layout uses names containing newline characters, uninstall can be convinced the user's
data directory is outside the folder it deletes, and then delete that data anyway.

## Details

**highest.** Source inspection; the data-loss route is traced in code, not executed. Confidence: likely — the unverified
premise is that the constructed dual-newline layout passes every guard (traced through each guard's consumption of the
corrupted spelling, not run on a live host). A proposed drop under the spec's deliberate-setup clause was REJECTED by an
independent fresh-context drop check (DISAGREE; verdict and grounds below), so the finding is recorded at highest with
the premise retained.

Mechanism at the reviewed commit:

1. `crates/farhelm-helm/src/provisioning/backend.rs:3332` — `resolve()` in `inspect_uninstall` runs
   `resolved=$(readlink -f -- "$1" 2>/dev/null | tr -d '\n')`. Command substitution already removes the trailing
   newline; the `tr` additionally strips newlines EMBEDDED in a resolved component, so the reported canonical/raw path
   is a corrupted spelling that may name no real folder.
2. Every downstream guard consumes that spelling: text containment in `crates/farhelm-helm/src/provisioning/plan.rs`
   (~963-991: `state.canonical/raw.starts_with(lib.canonical/raw)`, `under_lib` ~985, mount check ~1003) and the shell
   `-ef` ancestor walk (`backend.rs:3355-3366`) that yields `under_removed`.
3. For a layout where lib is reached through an ancestor symlink whose TARGET contains a newline and state through a
   different newline-containing bind spelling: the stripped spellings differ (text check passes), the stripped `-ef`
   ancestors no longer name real folders (identity check passes), and the mount check compares stripped spellings
   (passes) — so `plan_uninstall` accepts and `remove_directory`'s `rm -rf` (`backend.rs:~3656-3660`) removes the state
   data.
4. The code's own comment block (`backend.rs:3301-3320`) shows the identity walk was added precisely to REFUSE layouts
   text comparison cannot tell apart, citing SPEC.md's uninstall promise; "That identity only refuses … it never
   accepts." The `tr -d '\n'` turns a mandated refusal into silent acceptance. SPEC_impl.md (~8351-8356) likewise makes
   the identity walk "an identity answer it never uses to accept a path."

Spec analysis (independent drop check, DISAGREE with the proposed drop; commissioner verified the texts):

- The drop basis was SPEC.md:5571-5576 (deliberate-setup clause): "If the trigger takes a deliberate setup and the
  consequence follows from what that setup means, guarding against it is not Farhelm's job … a symlink whose target path
  contains a newline … The accidental guards need not handle those states." The trigger state IS one the accidental
  guards need not handle — that is why the drop was plausible.
- But the SAME paragraph's carve-out controls (SPEC.md:5578-5581): "This never … removes a protection this specification
  states for a specific case (such as … the installer keeping a user's file, or uninstall refusing unknown app-bundle
  entries), or accepts database corruption, a bricked installation or similar lasting damage." The defeated guard is
  exactly such a specific-case protection: SPEC.md:6023-6025 (home-alias uninstall exception) "It still refuses layouts
  it cannot tell apart safely"; SPEC.md:965 "Persistent user data is retained."
- Lasting damage is never accepted, trigger-independent: SPEC.md:2374-2375 "…similar lasting damage is never accepted,
  however unlikely the trigger." Deleting the user's state data is paradigmatic lasting damage.
- The "follows from what that setup means" test fails: a newline symlink means a path with a newline, not "delete my
  data"; the deletion follows from the `tr` bug defeating a mandated refusal, unlike the spec's proxy example where the
  consequence is inherent to the setup.
- The spec's own example pair is decisive: it accepts "every name the installer would use to keep a user's file already
  being taken" (5575-5576) yet preserves "the installer keeping a user's file" (5580-5581). Accepting a deliberate
  trigger state ≠ waiving a specific-case protection. Restoring this existing guard is not the excused "add complexity."

Verify: construct the dual-newline layout on a test host (lib reached through an ancestor symlink whose target contains
a newline; state through a different newline-containing bind spelling) and observe the inspection report non-containing
spellings and the plan accept; or unit-test `resolve()`'s output on a path with an embedded newline. Fix shape: fail
closed when `readlink -f` output contains an embedded newline (refuse the layout as indistinguishable) rather than
silently stripping it — restoring the mandated refusal, not adding new complexity.

Reviewer metadata (area 5 chunk B): suggested bucket: highest; confidence: likely (layout traced, not run);
proposed_drop: yes (deliberate-setup clause) — REJECTED by independent drop check D7 (DISAGREE), verdict retained at
/tmp/farhelm-tmp/399816ec-feef-4d89-89df-340af148359b/area5-dropcheck-D7/verdict.md; possible_cover: the
deliberate-setup clause (considered and rejected as above). Commissioner validation: mechanism confirmed at the cited
lines; coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
