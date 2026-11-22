# Locally invalid checkout names can start a recipe environment before refusal

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

The desktop can offer a sandbox launch for a checkout name that cannot produce a valid directory. The helm can create
the environment and attempt cleanup before telling the user to fix the name.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/sessions.rs:1388`.

the editable name is kept verbatim by launcher/draft.rs:1224–1234 and 1274–1280. launcher/request.rs:263–314 and 389–411
bypass remote preview for a new environment and do not apply static checkout naming checks. An exact title is serialized
into the request at 668–688. sessions.rs:1370–1430 verifies repo syntax, preview absence, conversation and title
agreement, but does not validate the title's printable spelling or the name derived from it.
provisioning/tensorlake_add.rs:346–360, 393–400, 606–640 and 1104–1126 registers and creates the environment before
sessions.rs:1887–1900 gets the first checkout preview. At service/core.rs:5462–5514 the supervisor rejects a nonempty
title with no usable slug or a basename over the cap via farhelm_proto::github_checkout::checkout_basename. Existing
direct naming assertions at github_checkout.rs:809–830 and 880–892 cover static refusal but not the pre-provisioning
boundary. SPEC.md:1820–1826 defers the exact host preview and accepts failed-launch cleanup for an actual destination
conflict. The name syntax and component length restrictions at 1828–1832 are determinable without that host. The parent
1e36b2ac090eeaa70c280c3ab4d58cbbc08bbe2f sessions.rs:1371–1401 already deferred these checks when a default root
existed, but blocked the same invalid request with no default before provisioning. The patch widens the path to such
requests in the previously blocked no-default case. Validate the first candidate's title and basename locally before
provider work, while keeping occupancy and canonical path on the target. Bucket: other based on the established failed
launch, extra provisioning and cleanup. No amount of billing or failed cleanup was observed. Confidence: confirmed by
source. Possible coverage: titled_names_follow_the_naming_rule and component length assertions for the supervisor's
verdict only. The same omission reaches the agent card on the typed agent path: agent_launch.rs:257–340 returns the
resolved name and repository separately. Agent_requests.rs:4247–4271 carries them into the plan. Lines 4482–4489 are its
checkout preflight before the approval. SPEC_impl.md:6318–6321 describes checking launcher eligibility before asking.
Existing preflight assertions in provisioning/tensorlake_add_tests.rs:657–704 cover an invalid initial prompt, missing
confirmation and missing snapshot with no provider work. They do not cover a locally unallocatable checkout name.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l16`.
