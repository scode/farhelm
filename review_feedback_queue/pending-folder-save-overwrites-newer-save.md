# An older pending save can overwrite a newer checkout folder

Reviewed commit: 126bbe267d31f256678f7b9a0466860ebd5a5c50

## TLDR

After closing and reopening Settings during a slow save, a newer successful save can silently revert to the older
folder.

## Details

Suggested bucket: `other`. Confidence: likely: the overlapping-write mechanism is confirmed by inspection; request
reordering under a naturally slow connection was not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial
review. Proposed drop: none.

crates/farhelm-ui/src/checkout_root.rs:28,46,75–96 keeps the saving lock in the component but launches each PUT in an
independent app-owned task. Settings dismisses freely (settings.rs:85–87), and reopening mounts a fresh unlocked field
(app_bar.rs:700–701). Trigger: delay save A before its server write, close/reopen Settings, successfully save B, then
allow A through. checkout_config.rs:78–81 applies A unconditionally; the old component suppresses its outcome, while the
new component retains B and its success message. The wrong global configuration persists until explicitly repaired.
Serialize saves across mounts or retain an app-owned pending-save lock. This affects folder configuration; stale
checkout previews are separately rejected.

Possible cover: SPEC_impl.md:482–489 requires saves to survive dismissal, but does not accept older saves overwriting
newer ones. No matching Planned item, BUGS entry, queue item or ledger decision found. The rare-glitch filters exclude
wrong durable state.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: checkout_root.rs:28,80–100 retains
component-local locking and independent saves; checkout_config.rs:78–81 remains unconditional. No fixing commit
identified.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.

## Additional affected scope

Second-pass classification: this is the same save-ordering finding, with no additional affected scope. Astra suggests
`high` for durable configuration rollback; Sol suggested `other`. Both are preserved for human triage. This repeat adds
no new distinct finding.

Reviewed commit: 126bbe267d31f256678f7b9a0466860ebd5a5c50

## TLDR

Settings can report that the new checkout folder was saved, then silently revert to an earlier choice.

## Details

Suggested bucket: `high`. Confidence: confirmed by static lifecycle and write-path inspection; request overtaking was
not reproduced. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/checkout_root.rs:26–32,74–95: the saving guard belongs to one mounted field, while its PUT
survives dismissal. Reopening Settings creates an unlocked field that can submit another PUT. If save A is delayed
before committing, save B can complete first, then A overwrites B. The old component suppresses its outcome, and the new
field never refreshes its initialized draft. crates/farhelm-ui/src/api.rs:434–443 sends independent requests;
crates/farhelm-helm/src/checkout_config.rs:68–85,281–323 has no ordering token or revision precondition. Preserve write
ordering across component lifetimes. Existing checkouts are unaffected; future checkout configuration is durably wrong.

Possible cover: SPEC_impl.md:482–489 requires saves to survive dismissal, but does not accept older saves overwriting
newer ones. Neither race filter covers a durable configuration rollback.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: checkout_root.rs:26–32,80–100 and
api.rs:519–527 retain the same behavior; the root PUT remains unconditional.

Review coverage: full commit diff read: true. No changed diff lines unread. TRIAGE_OUTCOMES.md was searched and relevant
material inspected, not read linearly in its entirety. Unchanged surrounding modules, other queue findings, and
unrelated later changes were not exhaustively reviewed. No Sol reports or working-copy queue additions were opened.
Truncated relevant output was reread in smaller selections.

Runtime evidence: None. Static review only; no browser, network-ordering, or filesystem reproduction. Applied the
supplied checklist to changed tests. No checkout or scratch files created, modified, or deleted; no fetch, rebase, or
helper agents.
