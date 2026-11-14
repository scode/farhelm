# Clone protocol docs reject the omission required by environment clones

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Clone protocol docs reject the omission required by environment clones

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g052/l6`.

### g052/l6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 13 (finding, open):

# Clone protocol docs reject the omission required by environment clones

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

The shared clone-request documentation tells future callers that a missing target host is only tolerated to decode and
refuse old requests. In this change, cloning a sandbox or container requires exactly that missing host. Following the
documented contract would construct a request that the new environment path rejects. This is an internal documentation
defect, not a runtime acceptance bug.

## Details

At crates/farhelm-proto/src/lib.rs:2064-2066, Clone is described as copying onto an explicitly named host. At 2077-2078,
host says: 'The target host's display NAME. `None` is retained only so an old wire shape can be decoded and refused.'
The complete contract added by the reviewed commit contradicts that exclusivity: SPEC.md:4327-4328 says
'`farhelm agent clone` of a sandbox or container session takes no `--host` or `--cwd`; it targets the source's recipe at
the source's place, with the source's launch and repository intent, never a copy of its disk.'

The executable boundary agrees with the new product contract. The same commit changes ParsedAgentVerb::Clone.host from
required to optional and routes a session-owned source through clone_environment_for_agent before requiring a host for
ordinary clone. At agent_requests.rs:4316-4321, any Some(host) is refused for an environment clone; absent host
continues through source validation and launch_environment_for_agent at 4368-4390. Thus omission is current supported
intent, not merely legacy wire tolerance. Document the two cases: ordinary clone requires the named target host, while
environment clone omits it and retains source recipe/place. The top-level Clone summary should carry the same exception.

Attribution was checked over the whole pinned range. Exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already had
the quoted host wording, which was then true for its ordinary-clone-only path. The 190d844e parent-to-child
agent_requests.rs patch introduces the optional parse and early environment branch, making the unchanged protocol
comment newly false inside the assigned range. Range endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same
host comment at lib.rs:1954-1955. The correct production refusal is not a refutation of this separately misleading
documentation.

Suggested bucket: other (internal protocol contract accuracy). Confidence: confirmed from pinned source, introducing
behavior patch, exact product contract and endpoint check; no runtime observation or reproduction. Could-be-highest: no
established security or loss-of-work route from the stale contract. proposed_drop: false. possible_cover: caller
supplied g090/c3:13 as this protocol-documentation handoff; do not count its separate fail-closed runtime concern as a
second new finding. Caveats: ordinary clones still require a host; this finding does not allege successful wrong-host
execution. All readily checkable premises resolved; l6 is closed as this finding.

Returned proposal (claims-033, event 13): **retain-verified**, suggested bucket **other**.

The protocol documentation says missing clone hosts exist only to decode and refuse old requests, while the newly
supported environment-clone path requires omission. Correct fail-closed execution does not refute inaccurate caller
documentation.

Original stronger consequence: Following the documented host requirement makes a valid environment-clone request fail.
No successful wrong-host execution, security effect or loss of work is alleged.

Unresolved premise or limitation: None for the documentation defect. Caller-supplied g090/c3:13 is a handoff, not proof
of a duplicate queue record.

Proposed basis: Record one internal protocol-documentation finding. Preserve ordinary clone's host requirement while
documenting the environment exception; parent may consolidate the handoff with g090 without duplicating its separate
runtime concern.

Existing parent disposition (event 13): verified. Parent explicitly accepted named retain-verified proposal from
claims-033 after inspecting it. The protocol documentation says missing clone hosts exist only to decode and refuse old
requests, while the newly supported environment-clone path requires omission. Correct fail-closed execution does not
refute inaccurate caller documentation. Record one internal protocol-documentation finding. Preserve ordinary clone's
host requirement while documenting the environment exception; parent may consolidate the handoff with g090 without
duplicating its separate runtime concern. Checked source/results:
["190d844e58c96cf197aa838d0b0eea8062f48435:crates/farhelm-proto/src/lib.rs:2064-2079: Clone summary requires an
explicitly named host and host=None is described as old-wire refusal
only.","190d844e58c96cf197aa838d0b0eea8062f48435:crates/farhelm-helm/src/agent_requests.rs:3871-3884 routes a
session-owned source before requiring an ordinary target host; 4313-4328 refuses Some(host), cwd and keys; 4368-4390
proceeds to environment launch.","The 190d844e parent/child patch changes parsed Clone.host from required to optional
and adds that branch. The quoted comment exists in exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a, when the new
behavior was absent; new behavior makes it false in
range.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:crates/farhelm-proto/src/lib.rs:1940-1956 retains the false
exclusivity. No runtime execution."] Current coverage/results: {"SPEC.md":"Agents and environments explicitly requires
environment clone to omit --host and --cwd; this establishes the contradiction, not acceptance of the stale
documentation.","SPEC_impl.md":"Agent requests and environments describes clone following source recipe/place; no
acceptance of the obsolete exclusive host rule.","TODO.md Planned":"Empty.","plans/queue":"No unlanded plans. Landed
environment-agent-operations is not future corrective coverage.","BUGS.md":"No matching known bug.","INDEX.md and
queue":"Searched Clone protocol/docs, AgentVerb::Clone and omitted/missing target host; no exact existing item.
clone-read-overwrites-manual-recipe-choice.md concerns a GUI draft race, not this wire
comment.","TRIAGE_OUTCOMES.md":"No active decision for this stale host contract.","FILTER.md":"Not a diagnostic/glitch;
this is persistent internal API documentation. No filter applies."} Exact cited cover: [] Original stronger consequence
and missing premise remain preserved: "Following the documented host requirement makes a valid environment-clone request
fail. No successful wrong-host execution, security effect or loss of work is alleged."; "None for the documentation
defect. Caller-supplied g090/c3:13 is a handoff, not proof of a duplicate queue record.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g052 record](records/review-through-00de0b6c/groups/g052.json). Record ID: `l6`.
