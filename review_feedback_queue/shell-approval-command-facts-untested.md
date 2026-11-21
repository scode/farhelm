# Shell approval tests omit the displayed shell and full command

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Approval tests check that the shell lacks a Permissions row but could miss a missing shell name or command.

## Details

**other**. Confirmed by source and contract inspection.

The test for Bash and Zsh approval cards would still pass if the card stopped identifying the shell or stopped showing
the command a user is asked to approve. Source-confirmed test gap: model_tests.rs:368-372 skips harnesses with no
offered permission, and :460-494 sends shell fixtures through create and restart but reduces every card to permission
chips and asserts only that the shell chip list is empty. No shell-specific fact assertion appears in the approval
modules or GUI integration-test paths at the reviewed commit. The new production branch is model.rs:731-738.
SPEC.md:4328-4335 requires inspection of the full launch command; :4347-4348 specifically requires the shell name and
command as well as the absence of the Permissions row. Cover the new branch with both real shell selections and positive
assertions for the shell name and full composed command, together with the existing absence assertion. The current
branch itself supplies both facts. Bucket other for test coverage. Possible_cover: none. No later approval files changed
through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f6`.
