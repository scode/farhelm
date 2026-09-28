# Provisioning replaces an existing tmux at its destination without checking who installed it

Reviewed commit: 7cc06814956a1e9b6ec41f29e2e57ec57b5d2178

## TLDR

When provisioning installs Farhelm's private tmux, it overwrites whatever file is already at the destination without
telling the user. After the destination fix below, that destination is Farhelm's own `~/.local/lib/farhelm/tmux`, so the
realistic loss is small; the missing ownership check remains.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0602-2b597e9-f96c`,
`F1 /
COR-TMUX-INTO-SHARED-BIN`). The main problem is fixed: UPDATE used to set the plan's private directory to the
registered binary's parent (`provisioning/plan.rs`, `plan_for_row`), so a binary at `~/.local/bin/farhelm` got Farhelm's
tmux installed as `~/.local/bin/tmux`, overwriting or shadowing the user's own. `plan_for_row` now overrides only the
binary path, and the private tmux always goes to the private directory.

What remains is the finding's secondary suggestion: the install step (`provisioning/backend.rs`, the payload install
around `chmod 755 tmp && mv -f tmp <destination>`) hashes the existing destination and replaces it when the bytes
differ, with no check of whether a `tmux` already there was put there by Farhelm. With the destination now inside
Farhelm's own directory, this only matters if the user placed their own file there. Refusing or surfacing the
replacement would need a way to recognize Farhelm's earlier install (for example a recorded digest), which is more than
a narrow change.
