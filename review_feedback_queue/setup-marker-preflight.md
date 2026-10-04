# Setup preflight can truncate a linked restart marker before unit publication fails

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Setup preflight can truncate a linked restart marker before unit publication fails.

## Details

`F56 / COR-SETUP-MARKER-PREFLIGHT` — **possible** — `crates/farhelm/src/setup.rs:747` — Setup preflight can truncate a
linked restart marker before unit publication fails

On Linux, `farhelm helm setup` records a pending restart beside a service definition when it is about to change a
running service. Setup now checks both services and writes their pending-restart markers before publishing either new
definition. Marker writing uses `std::fs::write` with empty contents, which follows an existing symlink and truncates
its target.

This changed ordering exposes a narrower failure case than the marker writer's pre-existing behavior. If both changed
services are active, the helm's marker is a symlink to an unrelated file, and publication of the supervisor's definition
then fails, the unrelated file has already been emptied. In the baseline, failure to publish that first definition
stopped setup before it reached the helm's marker. Successful baseline runs could already encounter marker collisions;
those cases are outside this net-change finding.

Record restart obligations without following occupied symlink targets, and refuse foreign marker shapes before writing
either obligation. A controlled fixture should combine the second-marker symlink with failure to publish the first
service definition, preserving the target file.

Suggested bucket: highest

Possible cover: `SPEC.md:1843–1849` excludes deliberate interference by processes under the same account. A nonmalicious
origin for this marker symlink has neither been established nor explicitly accepted. The generic marker-writing function
and path are unchanged from the baseline; only the changed-order failure variant is included here.

Caveats: The independent audit classified this record as ambiguous. The combination of a nonmalicious marker symlink and
first-definition publication failure is unverified, and no runtime reproduction was performed. Otherwise-successful
baseline marker collisions remain excluded.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_lifecycle p1 considered candidate`,
`drop_audit p4 narrowed changed-order remainder`.

Possible cover recorded during collection: SPEC1843–1849 deliberate local interference excluded, but nonmalicious
symlink origin not established or expressly accepted. Generic marker writer/path identical baseline; record only
changed-order failure variant..

Collection caveats: Independent audit ambiguous-record. Nonmalicious marker symlink plus first-unit publication failure
is unverified; no runtime reproduction. Scope does not include otherwise-successful baseline marker collision.
