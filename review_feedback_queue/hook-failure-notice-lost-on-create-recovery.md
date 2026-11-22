# A recovered live terminal can miss the setup failure notification row

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A recovered launch can keep its hook setup failure state while losing the notification that explains it.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-supervisor/src/service/core.rs:14718-14729,14758-14789,10496-10546,11654-11673,7068-7087;
crates/farhelm-supervisor/src/service/core/host_launch.rs:624-647;
crates/farhelm-supervisor/src/service/notifications.rs:397-435

A surviving agent can recover with a setup-failure explanation and no setup-failure notification. Reviewed commit
1a53d9d35889d7dde96e31320cc1681dd013737d, introduced by 47140bd63486498b7af024b05fa8ade7f8ea849e. The durable Failed
result keeps the reason, but the transient HookSkip carries the decision to record HookNotAdded. An unavailable initial
attach returns before the notification write; adoption and retention only load existing notifications later. No
notification row was deleted because it was never inserted. The disabled-policy branch also records Failed but
deliberately emits no HookSkip, so replay needs a durable distinction. HS-03 concerns the successful installation result
and deliberate clearing of an already persisted warning-eligibility bit, which is a separate mechanism and warning kind.
SPEC.md:3098-3103 and SPEC_impl.md:4512-4515. The reviewer suggested other, confirmed by source for the stated
adoption/recovery ordering. ; Possible coverage: distinct from hp-retained-create-disarms.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-supervisor/HS-01`.
