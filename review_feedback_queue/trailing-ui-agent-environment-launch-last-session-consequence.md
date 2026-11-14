# Last-session wording matches the later revision's changed deletion contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: considered whether saying deletion of the last session removes the environment understates
destruction of other sessions and files. Preserve the original destructive concern: if actual Delete still always
removed the whole resource without a separate warning, this wording could encourage a decision on a false safety
premise. The earlier 350a4b33 contract did always destroy the resource, but it is not the contract at this patch's
revision. At 190d844e, approvals.rs:1074-1106 puts the added wording in a creation card, not a Delete answer. Exact
later specification text for independent checking: SPEC.md:1504, "**Delete removes the sandbox together with its last
session.**" At :1514-1516, "The confirmation states both outcomes: the session and its checkout are deleted, and if it
is the last session in the sandbox, the sandbox, every process and file in it and its snapshots are deleted too, with
**Delete session**. It is never skipped, since it may destroy the sandbox." For Podman :4875-4877, "User Delete follows
Sandbox sessions' rule: it asks the container's Farhelm to delete the session and seal the container if no other session
remains, removes the container together with its last session, and otherwise reports that the container stays and how
many other sessions are in it. The confirmation states both outcomes and is never skipped." The disconnected/unknown
outcome fallback at :1526-1531 explicitly offers whole-sandbox Delete with the whole-resource consequence, and
:1535-1536 says host Delete sandbox always destroys it after its own confirmation; :4878-4884 gives the paral

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f2`.

### g077/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR: considered whether saying deletion of the last session
removes the environment understates destruction of other sessions and files. Preserve the original destructive concern:
if actual Delete still always removed the whole resource without a separate warning, this wording could encourage a
decision on a false safety premise. The earlier 350a4b33 contract did always destroy the resource, but it is not the
contract at this patch's revision. At 190d844e, approvals.rs:1074-1106 puts the added wording in a creation card, not a
Delete answer. Exact later specification text for independent checking: SPEC.md:1504, "**Delete removes the sandbox
together with its last session.**" At :1514-1516, "The confirmation states both outcomes: the session and its checkout
are deleted, and if it is the last session in the sandbox, the sandbox, every process and file in it and its snapshots
are deleted too, with **Delete session**. It is never skipped, since it may destroy the sandbox." For Podman :4875-4877,
"User Delete follows Sandbox sessions' rule: it asks the container's Farhelm to delete the session and seal the
container if no other session remains, removes the container together with its last session, and otherwise reports that
the container stays and how many other sessions are in it. The confirmation states both outcomes and is never skipped."
The disconnected/unknown outcome fallback at :1526-1531 explicitly offers whole-sandbox Delete with the whole-resource
consequence, and :1535-1536 says host Delete sandbox always destroys it after its own confirmation; :4878-4884 gives the
parallel container rules. These exact texts support accepting the new launch-card wording at its own revision. Suggested
bucket: other/no established defect. Confidence: source-confirmed comparison to the changed contract, not runtime
observation. Could-be-highest: original feared consequence was work destroyed beyond understood consent; no shipped
route established here because this is the creation card and actual destructive choices have their own specified
consequences. proposed_drop: exact later specification acceptance above, still proposed for independent check; no parent
drop accepted and no withdrawal recorded. possible_cover:none. No reproductions or tests run.

Returned proposal (claims-050, event 12): **covered**, suggested bucket **other**.

The launch-card wording matches the accepted last-session deletion contract at its own revision and recording main. It
is a creation card. The original feared destruction beyond a Delete warning remains a proposed refutation requiring an
independent check.

Original stronger consequence: A launch explanation might falsely imply other sessions protect the environment, leading
to destruction of other sessions and files beyond understood consent if actual Delete still always destroys the resource
without a separate warning. The history proposes specification acceptance, not an accepted parent drop.

Unresolved premise or limitation: The stronger concern needs a shipped destructive path that omits the actual
whole-resource consequence. That path was not established. This consultation does not certify every intermediate
executable implementation against the specification.

Proposed basis: Propose coverage only for the named creation-card wording under the contemporaneous accepted contract.
Preserve the original work-loss concern and the historical staging caveat for independent checking.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: contemporaneous-last-session-creation-card-contract-cover-only. Parent read full050. At190d
the creation card's last-session explanation matches the newly specified last-session seal/both-outcome Delete and
separate Whole fallback, including containers. Comparing350's older whole-only contract would misdate it.
Intermediate190d still has old whole-resource Delete copy; this does not prove every executable path already implemented
new contract, but neither establishes unwarned destruction. Proposed coverage only of named creation explanation, with
original possible misleading consent/other-session loss and staging caveat retained for fresh gate; no all-path source
certificate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f2`.
