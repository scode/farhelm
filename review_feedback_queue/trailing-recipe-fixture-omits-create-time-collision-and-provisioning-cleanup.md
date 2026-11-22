# Recipe fixture omits the create-time collision and provisioning cleanup

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

The no-default fixture checks numbering after a preview conflict and a successful create, leaving the specific
create-time lost-race cleanup unasserted.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions_tests.rs:502`.

sessions_tests.rs:489–502 returns Conflict only for a preview, scripts SessionCreated at 508–528, and calls the
post-attachment create helper at 562–571. A real supervisor test checks that create validation rejects changed occupancy
without touching the foreign file (service/core.rs:33106–33148). The existing Podman provisioning test asserts
resource/row cleanup and 'Not launched.' for a generic folder-create InvalidRequest
(provisioning/session_launch_failure_tests.rs:257–266, 380–385, 698–767). The existing-host launcher test at
checkout_tests.rs:1568 checks the ordinary re-preview policy. SPEC.md:1823–1826 instead says 'For a recipe target, a
destination conflict before session acceptance is a failed launch. It cleans up the new environment and shows Not
launched. with the conflict, rather than offering a preview for the destroyed host.' Source shows the serial preview and
create dispatch (sessions.rs:1887–1911) and pre-acceptance cleanup (provisioning/tensorlake_add.rs:1104–1129,
1145–1177). No combined assertion was identified for a no-default recipe checkout that conflicts after its successful
preview, sends no second checkout attempt and cleans up through provisioning. This differs from Provision/p1's
pre-launch stale task-handle race. Bucket: other. Confidence: established omission in the inspected assertions, no
production failure claimed. Possible coverage: direct supervisor occupancy refusal and generic provisioning rollback,
neither of which pins this combined result.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l8`.
