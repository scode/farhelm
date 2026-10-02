# Provisioning can lose its deadline while draining child output

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

A provisioning command whose direct process exits can still hold the host setup/update slot forever if a descendant
keeps an inherited output pipe open.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F16 / COR-PROVISION-DRAIN`, tagged **definite**.

At `crates/farhelm-helm/src/provisioning/backend.rs:1530`, `capture_child` bounds waiting for the direct child, then
calls `finish_child_output`, which awaits stdout and stderr drain tasks without the original deadline. A descendant that
inherited either pipe can keep it open after the direct child exits. The cleanup path also tries to derive the
process-group identifier from `child.id()` after waiting may have reaped the child. Payload transfer uses the same drain
helper.

A stuck setup or update can retain the host provisioning lock and one of the four run slots indefinitely. Retain the
process-group identity before waiting and bound child exit, output drains, and cleanup as one operation. Add a
regression with a descendant that keeps an inherited pipe open after its parent exits.
