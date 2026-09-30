# A host's "safe for YOLO" mark carries over to a different machine

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

The user marks a host (say, a throwaway VM) as safe for YOLO launches. Later that host entry reaches a different
machine: the address is recycled, the VM is replaced by a different install, or the user points the entry at another
machine. The user answers the helm's "this host now reports a different identity" prompt with "adopt". The entry is
still marked safe, so permission-skipping agents (Claude with skip-permissions, Codex `--yolo`, every Pi launch,
agent-requested creates and clones) now start on a machine the user never marked safe, with no confirmation. The hosts
panel keeps showing the entry as safe, so nothing signals the change. The confirmation exists to catch exactly this: a
YOLO launch on the wrong machine by accident.

## Details

Source: whole-codebase review, 2026-09-30. Two slices (helm host registry, helm store) found this independently.

Rebase note (main at 1ec60cc): a3b3866 renamed the setting. SPEC.md now says each host records "whether it starts YOLO
sessions without asking. Every host asks before YOLO launches until the user explicitly turns that off", and the control
is "start YOLO sessions here without asking" in the host settings dialog
(`crates/farhelm-ui/src/hosts/settings_dialog.rs:439`). The stored flag (`hosts.yolo_safe`), adoption and the guard are
unchanged, so the finding stands with "marked safe" read as "set to start YOLO sessions without asking". 35b41ff added a
second way to set it: the YOLO confirmation's "Start, and don't ask again on this host" calls
`api::set_yolo_safe(base, host, true)` (`crates/farhelm-ui/src/yolo_confirm.rs:118-119`) for the host id captured when
the refusal arrived, with no identity precondition, so a confirmation left open across an adopt or retarget sets the
mark on whatever install the row now points at. That is a second path for the stale-toggle variant below.

- SPEC.md (Topology, host settings paragraph around line 138): "Every host is sensitive (not YOLO safe) until the user
  explicitly marks it safe". The same paragraph defines a host by its supervisor-generated identity: "wiping and
  reinstalling a supervisor produces a new host identity", and on a different identity "the helm says so and asks
  whether to adopt the new host". SPEC.md (Topology) names "a wiped and reinstalled host, a recycled address" as the
  mismatch cases. An adopted identity is therefore a new host the user has not marked.
- The mark is a column on the registry row only (`hosts.yolo_safe`, schema v31,
  `crates/farhelm-helm/src/store.rs:1872`). `set_yolo_safe` (store.rs:4326; handler at
  `crates/farhelm-helm/src/hosts.rs:442-499`) documents the intent: "sensitivity is a property of the machine, not of
  how it is registered". The `yolo_guard.rs` module doc (lines 5-7) names the failure it guards against: "Starting an
  agent that runs without approval prompts on the wrong machine by accident".
- `yolo_guard::check` (`crates/farhelm-helm/src/yolo_guard.rs:63-91`; `Some(row) if row.yolo_safe => Ok(())` at line 80)
  decides from `row.yolo_safe` looked up by host id and nothing else, so every create, clone, replace, replace-with,
  restart-with, and agent `create`/`clone`/`ResolveProfile` against the row is admitted without the override.
- Adoption (`ConnectionManager::adopt`, `crates/farhelm-helm/src/manager.rs:2338-2424`, backed by
  `HelmStore::adopt_identity`, store.rs about 4615-4690) swaps `host_identity`, purges the session cache and, via the
  `host_identity <> ?2` deletes, the install-scoped history tables (`launch_history`, `folder_history`,
  `create_history_sessions`, …), and resets `cache_truncated` (store.rs:4658:
  `UPDATE hosts SET host_identity = ?2, cache_truncated = 0 WHERE id = ?1`). It does all of that explicitly because the
  old install's facts must not be attributed to the new one, but it never touches `yolo_safe`. `grep yolo_safe` in
  `crates/farhelm-helm/src` finds only the setter, the guard, and the hosts view.
- A retarget keeps the mark too. `HelmStore::update_ssh_destination` (store.rs 4226-4237, "What a retarget leaves
  alone") changes only `destination`. For a row with no recorded identity yet (added while the machine was off), a
  retarget to another machine records that machine's identity at first contact and it inherits the mark silently. For a
  row that has an identity, a retarget to another machine freezes as identity-mismatch, and the one-click adopt finishes
  the job.
- The UI's adopt action (`crates/farhelm-ui/src/hosts.rs:2122`, "adopt <identity>") says nothing about carrying the YOLO
  setting over.
- Contrast: checkout-config per-host overrides deliberately survive adoption (`checkout_config.rs` module docs,
  "Overrides are config for the REGISTRY ROW"), but that is written down as a binding choice; for `yolo_safe` the spec
  and the setter's own doc say the opposite.
- A narrower variant with the same root: `set_yolo_safe` carries no precondition on the destination, identity or
  incarnation the user was looking at, so a toggle from a stale settings dialog can mark whatever machine the row points
  to after a concurrent retarget from another client. Rare; it only matters because of the above.

Trigger: mark entry safe → the entry reaches a different install (recycled address, reinstall, or a retarget) → adopt
(or first contact on a never-contacted row) → any YOLO launch on that host id skips the confirmation SPEC.md ("A YOLO
launch on a sensitive host … needs an explicit confirmation") requires.

Verify: store test that adds an ssh host, runs `record_first_contact` with identity A, `set_yolo_safe(true)`,
`adopt_identity` to B, then `list_hosts()` still shows `yolo_safe == true`. Or a REST-harness test: mark a host safe,
script a mismatch, adopt, and check that a YOLO create is not refused with `YoloOnSensitiveHost`.

Fix shape (needs a spec decision): bind the mark to the install. Either clear `yolo_safe` in `adopt_identity`'s
transaction (and when first contact records an identity on a row that was marked safe before it was ever contacted), or
store the identity the mark was granted under and treat a mismatch as sensitive. Say which in SPEC.md's host settings
sentence. Whether a bare retarget that reaches the SAME identity should keep the mark is a separate spec question;
keeping it matches "an SSH destination can be corrected without touching the host's identity". If carrying the mark
across a wipe-and-reinstall is intended, the adopt prompt should say so, and a recycled-address adopt then needs its own
answer.
