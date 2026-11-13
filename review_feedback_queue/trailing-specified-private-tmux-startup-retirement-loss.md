# Legacy terminal removal can end running work; specification explicitly accepts it

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: after this upgrade, sessions left in old tmux terminals lose their running processes and terminal screens; the supervisor no longer offers the old Stop, attach, or resize paths.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g125/c1`.

### g125/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: after this upgrade, sessions left in old tmux terminals lose their running processes and terminal screens; the supervisor no longer offers the old Stop, attach, or resize paths. Details: the assigned handlers patch removes foreign-pane ownership probing from Stop (new lines 865-906), all tmux attach setup (1696-1966), and tmux resize operations (2076-2084). The shipped route is verified in source: service/core.rs lines 6142-6151 invoke OldTerminals::end_private_server for the state-directory owner before reload; service/core/old_terminals.rs lines 162-164 invoke end_server, whose lines 223-251 terminate the verified server and remove its files; service/core.rs lines 6675-6679 no longer look up a tmux row's address and give it no terminal. This establishes deliberate process termination and loss of the server-held terminal state, not merely a hypothetical client capability failure. Could-be-highest: yes, this terminates user-owned processes and discards terminal state. Suggested bucket: highest (loss of work), retained for the parent's independent specification-acceptance check. Confidence: confirmed source/contract analysis; no execution. proposed_drop: explicit specification acceptance only, not rarity or lack of impact. Exact acceptance, SPEC.md at this commit, Terminal experience, lines 3006-3007: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program.' Lines 3008-3010 retain ended-agent outcomes but discard their screens and close old tabs; lines 3014-3016 also explicitly accept a cleanup failure allowing Restart beside an old surviving agent. SPEC_impl.md lines 2210-2235 limit the terminal switch points to host runtimes and grid attaches. possible_cover: parent supplied likely analogous records g099/g113/g118/g130; no coverage sources searched and no duplicate disposition asserted. Caveats: the retirement is deliberate per the quoted contract, so this record asks for independent acceptance verification, not a behavior reversal. Startup source was inspected only to validate this removal's shipped premise, not for an independent review of that group's implementation. This finding revision supersedes the earlier candidate classification and adds the completed source checks. No parent drop accepted.

Returned proposal (claims-079, event 9): **covered**, suggested bucket **highest**.

The process/screen loss is real by source and explicitly accepted by the product contract. This proposal is exact specification coverage, subject to the parent's independent drop check because user-owned running work is ended.

Original stronger consequence: Terminates user-owned processes and discards old terminal state. Earlier candidate labels are superseded by a finding explicitly retained as highest for independent specification-acceptance checking. No parent drop was accepted.

Unresolved premise or limitation: None for this deliberate retirement consequence. Startup retirement's correctness against other processes is a separate question, not reviewed here.

Proposed basis: Exact specification acceptance of old-private-server retirement only. Parent-supplied g099/g113/g118/g130 analogies were not inspected or silently treated as aliases because they are outside this batch.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g125 record](records/review-through-00de0b6c/groups/g125.json). Record ID: `c1`.

