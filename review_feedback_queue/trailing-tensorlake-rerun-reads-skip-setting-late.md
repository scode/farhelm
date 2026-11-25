# Tensorlake setup Re-run reads "create without asking" after the plan arrives

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When the user presses Re-run on a failed Tensorlake sandbox host while "Create Tensorlake sandboxes without asking" is
off, they expect the "Create Tensorlake sandbox" confirmation, which warns that the sandbox bills. If that switch is
turned on while the plan is being prepared (from Settings or another client), Farhelm creates the billed sandbox without
asking. The same commit fixed this for ordinary SSH setup but left the Tensorlake path reading the setting late.

## Details

Concerns 7c2cfe02db ("fix: keep host actions tied to what was shown", #4849); the Tensorlake behavior itself is
pre-existing. Locations: `crates/gui/farhelm-gui/src/hosts/setup.rs:804-811` (the Tensorlake arm of `rerun` calls
`ask_or_create`), `setup.rs:863-888` (`ask_or_create` issues GET /api/tensorlake when the plan arrives and submits if
`create_without_asking`, decided at line 881), `setup.rs:890` (`settle_sandbox_rerun` submits without a question when it
skips); `hosts/menu.rs:419-421` (Re-run's drawn `asks` is computed only for SSH).

7c2cfe02db's stated rule is that "a later preference change cannot skip the confirmation already required for that
attempt". It threads the press-time SSH choice through `setup::rerun` (`asks_before_setup`), but the Tensorlake arm
ignores it and calls `ask_or_create`, which issues a fresh account read after the probe settles and submits the plan
without a question when `create_without_asking` is true. Nothing records what the setting was when the user pressed
Re-run, so a press made under "ask" can end in a billed POST /api/hosts/provision with no confirmation. The probe window
is real: it goes through `setup_sign_in::plan`, which can wait on sign-in. SPEC.md's Add host paragraph (around lines
426-427) says only that the setting skips the confirmation "including for setup reruns", and SPEC_impl.md (around lines
6819-6826) says Rerun "consults only that acknowledged setting" and that "a cancelled or replaced opening cannot
authorize a later billed POST"; neither says the setting is read at plan arrival rather than at the press.

The Add host Tensorlake form (`hosts/add/tensorlake.rs:393-425`) decides from the form's own account read, not a fresh
read at plan arrival; it is outside this commit's hunks and not part of this finding.

Verify by reading the `SetupRerun::Tensorlake` arm of `rerun` and `ask_or_create`. Fix: capture the account's
acknowledged `create_without_asking` value at the press (the menu would need to show or read it then), and require a
question if either the press-time or the arrival-time value asks; a read failure already asks.

Bucket: other. A billed sandbox creation the user explicitly requested via Re-run proceeds without its confirmation in a
narrow race with a settings change; the user did request the re-run, and the switch was turned on by a user action. Not
highest: no data or credential loss; the cost is a sandbox the user asked to re-create.

Confidence: confirmed from source that the setting is read after the plan arrives; the race requires a settings change
during the probe.

Possible cover: the rerun sentence in SPEC_impl.md, "Adding Tensorlake hosts" ("Rerun retains the Tensorlake plan kind,
consults only that acknowledged setting"), describes which setting is read, not when; it does not cover this.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g22 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
