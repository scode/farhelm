# Repository selection leaves an earlier gh: search blocking Launch

Reviewed commit: dccdb7449698c59175ec8d90d1dae1f881f4494d

## TLDR

Choosing a valid repository in the new destination field can leave Launch disabled despite a successful checkout
preview.

## Details

Suggested bucket: `other`. Confidence: confirmed by static control-flow inspection; no browser reproduction. Source:
dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

At crates/farhelm-ui/src/list/create_form.rs:3684–3700, select_repository accepts the repository but leaves
composer_search unchanged. Trigger: enter gh:anything in global search, switch to managed checkout, then type and select
a valid owner/repo in the dedicated field. The preview succeeds, but lines 3091–3093 still classify the lingering search
as an unmet prerequisite; submission also returns at 4163–4165. Unlike global-result acceptance at 3676–3678, the new
selection never clears the competing search. The user must discover that clearing the earlier search enables Launch.
Clear or reconcile that search when accepting the dedicated repository selection.

Possible cover: SPEC_impl.md:113–117 accepts gh: priority for discovery queries, but does not accept blocking launch
after explicit repository selection. No matching Planned item, BUGS entry, queue item, ledger entry or filter found.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:3849–3866 still leaves
search unchanged; 3219–3221 and 4447–4449 retain both guards.

Review coverage: full commit diff read: true. No commit diff content unread. Truncated create_form output was reread in
smaller chunks. The oversized ledger search was replaced with narrower searches; unrelated ledger bodies were not read
exhaustively. Unrelated queue finding bodies, unchanged portions of the large modules and existing tests, and the full
downstream supervisor allocation implementation were not read. No Sol reports or working-copy queue additions were
consulted.

Runtime evidence: None; static review only, as requested. No files created, modified or deleted; no fetch, rebase or
helper agents.
