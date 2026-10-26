# Offline uninstall still has an unresolved surviving-tmux route

Reviewed commit: 728254e49e5b88756269991dae0af76dc0cb7c0b

## TLDR

The offline retry can omit the session check, but a supported history leaving the unit file gone, unit stopped,
supervisor absent and private tmux alive remains unestablished. Active-unit repairs do not explicitly accept losing
surviving sessions.

## Details

Suggested bucket: **other**. Confidence: Conditional lead: the source or causal premise named below remains
unestablished. No reproduction was performed.

Source location: `crates/farhelm-helm/src/provisioning/service.rs:1467-1483`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Later-turn synthesis: independent exclusion verdict remains AMBIGUOUS; proposed_drop no. Reviewed
728254e49e5b88756269991dae0af76dc0cb7c0b. g259 retracts its earlier likely-accepted-exception rationale and stops
focused static investigation incomplete. Gate admission and backend kill-server remain established conditional on gone
unit file, absent supervisor and inactive/failed unit with unchecked live tmux. No complete supported history proves
this conjunction despite Restart=on-failure and reload/control-group cleanup. Initial successful uninstall confirmation
checks sessions; any later live session must be placed in a particular failed-attempt history. OTHER on established
evidence; possibly highest only if that complete route is established. Possible exact-moment/two-rare-fault acceptance
remains unproven for the unspecified history; active-unit repair and ledger out-of-scope are not acceptance. No runtime
experiment, invented history, or whole-finding exclusion. The narrow absent-leaf and newline refutations protect
separate conditions. Possible cover: exact-moment and coincident-fault acceptance is unresolved for this unspecified
history; the active-unit repair does not settle the surviving-tmux remainder.

Provenance: `a038/f4`, `g259/f1`, `g210/f4`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
