# A superseded mounted launcher may retain an earlier conversation selector

Reviewed commit: 788b63e12c5437ae2a3ee8c27dcc4cc8538e63e4

## TLDR

If a supported action can supersede an edited Clone or Replace-with source while its launcher stays mounted, the new
source retains the earlier conversation selector and could send work into that conversation. The retention is confirmed;
an accessible shipped action completing that sequence was not established. This remains a conditional lead.

## Details

`crates/farhelm-ui/src/list/create_form/draft.rs:557–560,1321–1364` retains the selector during source reseeding;
`642–655` projects it into structured launch intent. Managed checkouts refuse a named conversation
(`list/create_form.rs:4919–4925`). Equivalent behavior predates the extraction at
`67fb10767895037d4d63d09d0f7c6bdb43c6fe85`, `create_form.rs:3082–3089,5034–5043`.

**Missing source premise:** a shipped source-opening action after selector entry that supersedes this mounted draft.
Ordinary row/header/card controls lie outside modal isolation; Cmd-N and Cmd-K yield while a modal exists (`788b63…`,
`lib.rs:1547–1556,1577–1589`), and template shortcuts retain the current opening (`list/view.rs:3253–3258`). Direct
source-owner tests prove retention, not UI reachability (`draft.rs:1715–1753,1784–1793,1890–1922`). Same-generation late
host reconciliation preserves the current literal draft (`1817–1863`); that alone is not cross-opening inheritance.
Cancelling and opening anew unmounts the draft and initializes an empty selector (`list/view.rs:3903–3913`;
`draft.rs:119–153,1476–1489`). Those narrow routes are refuted.

Suggested bucket: **provisionally highest** for possible unintended conversation work. Confidence: **confirmed
retention/projection, source-incomplete harmful route**. The selector stays visible and editable. No harmful work,
occurrence or regression introduced by extraction is claimed. The primary reviewers withdrew the ordinary-trigger
examples; independent review agreed only with those narrow barriers and found a full exclusion ambiguous.

Possible cover: `SPEC.md:649–657,1218–1228,1238–1251` does not expressly accept cross-opening retention. The complete
`TRIAGE_OUTCOMES.md:17637–17656` transfers the different Guarded/render-time replacement-source concern. No exact
Planned, BUGS, queue or filter coverage was found. Proposed drop: none for this unresolved remainder. Establish the
shipped route before treating retention alone as a bug or designing a reset that discards intentional same-draft input.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g027/c1`, withdrawn `g041/f1` and `g042/f1`, fresh `a012/g041-f1`. These
investigations stop incomplete on the accessible-action premise. No runtime reproduction.
