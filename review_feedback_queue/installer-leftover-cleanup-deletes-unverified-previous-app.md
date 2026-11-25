# Installer leftover cleanup deletes a displaced "previous" app it never verified it owns

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

After an interrupted app install/update, the next install or uninstall can permanently delete an app it never built —
including a foreign app someone placed at ~/Applications/Farhelm.app, or the user's only previous Farhelm installation —
instead of restoring it or leaving it alone.

## Details

**highest.** Source inspection; commissioner confirmed both code paths and the broken restoration promise. Bucket
judgment (the reviewer explicitly deferred it): recorded at highest because the established code path deletes an app
bundle the installer cannot show it built, violating SPEC.md's ownership rule ("a file is not destroyed merely because
its name matches"; "The installer changes Farhelm.app only when it can show it built it") and the code's own invariant
("Finder can put another app at the place at any moment, and that one must neither be deleted with the previous app nor
be replaced"). The consequence class includes irreversible loss of a user-placed foreign app (not assumed recreatable).
Uncertainty retained: the foreign-app deletion needs two Finder-style swaps racing the rebuild (rare, not observed); the
single-swap/publish-failure variant deletes only the user's own prior Farhelm install (reinstallable, session data lives
in the state dir) and alone would bucket high. The design explicitly anticipates Finder swaps, so the trigger is within
the spec's concurrent-and-interrupted-runs contract, not exotic deliberate setup. Per the err-higher rule, recorded
highest.

Mechanism at the reviewed commit:

1. `crates/farhelm-supervisor/src/installation/install.rs:1008` moves whatever is at Farhelm.app into stage/previous;
   1018 compares its device+inode to the identity captured at inspect; on mismatch it tries a NOREPLACE put-back
   (1021-1031). The comment at 1019-1020 claims a failed put-back "stays in the staging folder, which the next run
   restores from."
2. `remove_leftovers` (install.rs:1249-1268): a staging folder whose children are all version/Farhelm.app/previous is
   treated as the installer's; `previous` is put back ONLY when the app's place is `EntryKind::Absent` (1249-1250).
   Otherwise the folder is flushed and `dirs::remove_tree` deletes it, `previous` included (1268), with no ownership
   check on `previous`.
3. The restoration promise breaks exactly in the case it was written for: the put-back fails BECAUSE a second app
   arrived at the place, so the place is occupied, so the next run deletes instead of restoring. The publish-failure
   path (1048-1060) can likewise leave the user's own prior install in `previous` to be deleted by the next run.

This defeats SPEC.md "Installation and updates" (the ownership rule above) and the "Concurrent and interrupted runs"
recover-or-restore contract. Verify: drive `remove_leftovers` on a held ~/Applications fd containing
`.farhelm-stage.<32hex>` whose only child is `previous`, with a different folder at Farhelm.app; observe `previous`
deleted. Fix shape: before deleting a staging folder holding `previous`, establish the displaced app is one this
installer may remove (matching record/id, or the place now holds the bundle this module published); otherwise keep it
and refuse, or preserve it under a visible name — never delete merely because the place is occupied.

Reviewer metadata (area 5 chunk F, supervisor-side installation): suggested bucket: highest (single-swap variant high;
bucket judgment deferred to commissioner, who kept highest with the uncertainty above); confidence: confirmed code path,
rare double-swap trigger not observed; proposed_drop: none; possible_cover: none (the cited spec sections are the rules
violated, not a cover). Commissioner validation: both paths confirmed at the cited lines; coverage searched (queue,
TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
