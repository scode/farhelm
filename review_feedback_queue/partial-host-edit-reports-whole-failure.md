# A failed host edit can leave the new alias saved

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A single approved host edit can change the alias and then fail to change the icon or color. The failure message can
leave out that the alias already changed.

## Details

Bucket: **other**. GUI migration final whole-tree review, R8 Sol partial host edit.

Source references: `crates/farhelm-helm/src/agent_requests.rs:3478, 3501`,
`crates/farhelm-helm/src/hosts.rs:529, 1037, 1086–1105`.

The agent edit takes host locks and performs the alias mutation and reconcile before the appearance mutation. If
reconcile fails after the alias commit, or the appearance write fails, the handler returns an error for the combined
edit. There is no combined transaction or partial-result reporting for the later failure.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. The user authorized the
saved alias. The concern is a partly applied change reported as a whole failure. SPEC.md “Errors and diagnostics” asks
for a concrete actionable error. FILTER.md's partly failed session operations covers session status and history, rather
than durable host edits.

Commit the alias and appearance together where possible. If reconciliation can fail after the commit, report which
fields took effect and where it failed. Test an appearance-write refusal after an accepted alias change.
