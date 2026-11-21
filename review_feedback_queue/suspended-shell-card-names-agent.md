# Suspended shell cards describe a preserved agent command

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A suspended shell’s card says an agent command is preserved.

## Details

**other**. Confirmed by source and contract inspection.

Source-confirmed: session_pane/unavailable.rs:164-186 derives the notice from the row's host without examining its
launch; :299-337 uses `ui.suspended_explanation()` for a suspended session-owned environment. Environment_ui.rs:690-699
says `including the agent's running command` for both providers. SPEC.md:1620-1625 requires a Session suspended card
explaining preserved processes, :5137-5141 applies that pattern to containers, and :5568-5569 distinguishes shell
sessions from agents. The broader explanation accurately says the environment's processes are preserved and continue on
Resume, and the offered Resume targets the host from the same notice. Bucket other for wording. This is separate from
shell-permissions/f3 (Hosts' question before suspending) and shell-presentation/f6 (partial failure after Stop and
suspend). Possible_cover: none. No later provider-wording change through pinned tip
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f10`.
