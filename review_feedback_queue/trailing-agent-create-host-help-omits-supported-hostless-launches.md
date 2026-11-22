# Agent create host help omits supported hostless launches

Reviewed commit: df6199142d6495a95b2d008400ba2686efb775c6

## TLDR

Agent create's Host help can send callers looking for an unnecessary flag when a template launches a recipe or their
session is already inside a Farhelm environment.

## Details

**other.** Source inspection. Main reference `crates/farhelm/src/main.rs:335`.

main.rs:335–337 says --host is required unless a template sets the host or --recipe is given, and always with an
idempotency key. SPEC.md:4734–4736 allows an environment's own agents to use a hostless create, and :4756–4762
explicitly allows a recipe template. agent_requests.rs:1990–2008 dispatches a recipe plan before the ordinary host
rules, and :2023–2028 limits the host requirement for keyed requests to the ordinary fleet path, excluding
environment-confined creates. :2483–2500 resolves spawn and confined destinations on the owned host. :14694–14725
asserts hostless create dispatch for both providers. Clarify the host and keyed-host help for those cases. Bucket: other
(discoverability). Confidence: source and contract. Possible coverage:
recipe_flags_send_the_environment_verb_and_refuse_placement_mixes checks folder help and placement conflicts, without
asserting the Host help.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `notices/f5`.
