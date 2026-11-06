# Automatic update opt out can roll back

Reviewed commit: 771864ac3f38334a8191a424d855669efddb089a

## TLDR

A power loss can roll back opting out of automatic updates and allow later unattended updates. Whether the general
preference-loss acceptance covers this authorization remains unresolved.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/store.rs:5596;preferences.rs:249-273 at 771864ac3f38334a8191a424d855669efddb089a`.

Source assessment (read with the later corrections below) (review record g076/p2l2, event 35):

Reviewed commit 771864ac3f38334a8191a424d855669efddb089a, checked through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: After the user turns off automatic remote-host updates and the save
succeeds, an OS crash or power loss can restore the old on setting; restarting the helm can then update remote hosts
without another opt-in. Source mechanism: update_preferences chooses the critical sync barrier solely from
patch.tunnel_consent.is_some() (store.rs:5596; pinned 5697). An auto_update_remote_hosts-only false/null patch therefore
commits at ordinary WAL/NORMAL. preferences.rs:249-273 owns this write, orders it with automatic_update_admission at
257-263, and returns 204 only after success, so normal process-lifetime ordering is correct; the omission is power-loss
durability, not an admission race. A baseline true value can already be durable (e.g. an earlier checkpoint) before the
later NORMAL off commit is lost. Production AppState installs AutomaticUpdates::start at lib.rs:640. automatic.rs:43-64
starts with an initial scan and then listens to fleet revisions; 103-111 reads the stored preference, 123-143 selects
eligible older/old-protocol remote hosts and plans/starts their normal updates. start_automatic_update in
service.rs:1395-1415 rereads the same restored Some(true) under its policy gate and claims the run; no new user answer
is required. Source remains at pinned tip: preferences.rs and automatic.rs have no changes over the reviewed tail;
pinned service.rs:1398-1418 retains admission and store.rs:5697 retains tunnel-only FULL selection. Product contract:
SPEC.md Topology:145-154 at reviewed commit (pinned 145-154) says automatic updates are opt-in and off prevents new
automatic starts, including planning still in flight. SPEC.md Security:3715-3718 (pinned Updates are never forced
paragraph starting 3845) requires explicit Update or the off-by-default automatic setting. SPEC_impl.md:277-279 says
only true authorizes and the value survives reopening; neither specific policy section states that an acknowledged
opt-out may be restored by power loss. Suggested bucket other: established consequence is an unwanted normal remote
update, not attacker-selected execution, data loss or a shown severe availability failure. Do not upgrade to highest
merely because provisioning executes remote operations; the normal update's own trust and refusal checks remain.
Confidence confirmed source-level route conditional on the documented possible loss of a recent NORMAL commit, not an
observed power-loss experiment. proposed_drop false. Exact possible_cover for parent: general ordinary WAL/NORMAL
recent-commit-loss acceptance in SPEC_impl.md at introduction 5100-5107 / pinned 5138-5145, plus 'Unrelated preference
patches remain NORMAL' at introduction 5111-5112 / pinned 5149-5150. The automatic-update field is listed as a shared
preference at introduction 248-279, but it is also explicitly an execution authorization; the text does not name this
opt-out in its critical setter list or explicitly accept restoring it. Parent should decide this exact specification
boundary rather than silently importing either f2's severity or a generic acceptance. This is distinct from f2's
restored removed-host command authority, f1's accepted Mac cache limit and d4's unresolved first-run ancestry.
possible_cover elsewhere unknown; no queue/plan/filter or other group inventory searched. Suggested fix if not accepted:
include present auto_update_remote_hosts patches in the critical sync condition, so acknowledged opt-outs survive on the
same supported durability boundary as other trust-changing preferences. Caveats: the execution-authorization preference
and its NORMAL write predate the assigned setter-only fix; this is a related omitted authorization path, not a new code
regression. No runtime tests, builds, network/process experiments or live actions. Remaining pass checks will finish
before returning.

Recording coverage: General NORMAL preference-loss acceptance and unrelated-preference wording may cover this opt-out,
but restoring an execution authorization is not expressly named. Retain possible cover for triage.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g076/p2l2`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
