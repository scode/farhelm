# Repository search reports no matches before its reply

Reviewed commit: ea155b56def08bc034f4c2a92da254d9a0c6022c

## TLDR

An ordinary partial repository search says there are no matches before the host has answered. Matching results can
replace that message later.

## Details

Suggested bucket: **other**. Confidence: **confirmed source mechanism**; response latency and visible duration were not
measured. No wrong-target operation or user-work loss is established.

`crates/farhelm-ui/src/list/create_form.rs:4398–4403,6774–6782` renders the empty message whenever the suggestion list
is empty. Discovery first waits 150 ms (`4281–4294`), then awaits the host; old-query replies cannot fill the current
list (`4297–4305`). `github_checkout.rs:202–217` supplies an immediate manual choice only for a valid full owner/repo,
so normal partial-name edits pass through an empty list while discovery is pending.

The reused helper's repository branch unconditionally reports no matches for nonempty input
(`launch_composer.rs:947–948`); its loading arguments concern other search categories. Represent repository discovery's
own pending state and reserve the negative conclusion for a completed matching response.

Possible cover: slow-host acceptance permits latency, not this premature conclusion. Display lag after external edits
has a different trigger. The rare-diagnostic filter does not fully match ordinary query editing. No exact Planned,
known-bug, queue or ledger coverage was found. Proposed drop: none.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g061/f1`.
