# Hosts describes a working shell as an agent when asking to suspend

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Hosts calls an active Bash or Zsh session an agent when asking to suspend.

## Details

**other**. Confirmed by source and contract inspection.

Source-confirmed: hosts/offers.rs:380-389 offers Suspend on a connected provider host. Environment.rs:503-517 and
:252-265 ask on any fresh Running session without inspecting its launch. SPEC.md:5568-5569 gives shells Running status.
SPEC.md:1620-1625 and :5137-5141 apply Suspend to the whole environment, and :350-355 describes the host activity check
and preserved processes. Hosts composes "An agent on" with "is working, or its status could not be checked"
(hosts/lifecycle.rs:67-69, :175-193). With a complete connected listing whose only Running row is a shell, the warning
refers to a nonexistent agent in either clause. The provider help still explains the host-wide freeze
(environment_ui.rs:675-685), so the consequence is wording, with no change to consent or process scope. Bucket other.
Possible_cover: SPEC.md:351-353 calls the pre-shell case an agent working, while basing immediate Suspend on conclusive
non-working statuses. That wording does not describe shell-only rows. No later changes to these Hosts or environment
confirmation paths through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f3`.
