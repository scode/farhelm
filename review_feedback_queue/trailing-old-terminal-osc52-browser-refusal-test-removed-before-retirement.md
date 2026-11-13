# Old-terminal clipboard read refusal loses its browser check before retirement

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: the rewrite preserves the grid clipboard-read test but removes the useful old-terminal browser assertion while adopted old terminals are still supported at this intermediate revision. That is a test-only coverage loss, not evidence of a clipboard leak. This corrects the initial candidate's reliance on absence of a current functional regression to dismiss a removed test. Source: original terminal-clipboard.spec.ts:357-456 contains an old branch that waits for an OSC52 response, requires its base64 payload to be exactly empty, excludes the literal/base64 clipboard secret and confirms the system clipboard is unchanged. Reviewed terminal.js:839-846 implements the distinct old-provider security boundary with readText returning an empty string; :3998-4025 installs that custom provider into the xterm addon. SPEC_impl.md:1823-1839 describes the clipboard/read boundary, while2294-2311 still adopts and serves earlier tmux terminals at this revision. The retained grid branch at reviewed370-425 instead gates a query, waits for its parsed marker and checks no page report is sent. The supervisor emulator's queries.rs:103-128 and that browser branch do not exercise the old page addon's provider. Later1ed482dde220452e424973faf38a5e1318986333 ends old terminals and f560058da19e02716e6f943883a9508b447aff4e removes xterm from the page; tip SPEC_impl.md:1728-1745 places read refusal in the supervisor. Therefore this is an intermediate-only old-provider test gap and ceases after actual retirement; it is not a request to resurrect the old renderer now. Suggested action at the reviewed intermedia

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g174/c2`.

### g174/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; later retirement checked through00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the rewrite preserves the grid clipboard-read test but removes the useful old-terminal browser assertion while adopted old terminals are still supported at this intermediate revision. That is a test-only coverage loss, not evidence of a clipboard leak. This corrects the initial candidate's reliance on absence of a current functional regression to dismiss a removed test. Source: original terminal-clipboard.spec.ts:357-456 contains an old branch that waits for an OSC52 response, requires its base64 payload to be exactly empty, excludes the literal/base64 clipboard secret and confirms the system clipboard is unchanged. Reviewed terminal.js:839-846 implements the distinct old-provider security boundary with readText returning an empty string; :3998-4025 installs that custom provider into the xterm addon. SPEC_impl.md:1823-1839 describes the clipboard/read boundary, while2294-2311 still adopts and serves earlier tmux terminals at this revision. The retained grid branch at reviewed370-425 instead gates a query, waits for its parsed marker and checks no page report is sent. The supervisor emulator's queries.rs:103-128 and that browser branch do not exercise the old page addon's provider. Later1ed482dde220452e424973faf38a5e1318986333 ends old terminals and f560058da19e02716e6f943883a9508b447aff4e removes xterm from the page; tip SPEC_impl.md:1728-1745 places read refusal in the supervisor. Therefore this is an intermediate-only old-provider test gap and ceases after actual retirement; it is not a request to resurrect the old renderer now. Suggested action at the reviewed intermediate step: preserve the security regression assertion until its consumer is removed, or carry its browser/custom-provider oracle through an adopted-old fixture. Suggested bucket: other, harm established is confined to regression testing. Confidence: confirmed removal, distinct mechanisms and temporal scope by source analysis; no runtime behavior observed. proposed_drop: false; prior proposed-drop rationale withdrawn. Could-be-highest: an actual clipboard leak would be security-impacting, but none is shown or claimed; removed tests alone do not establish highest. possible_cover: related intermediate old-renderer coverage losses c3/c4, but this is the distinct OSC52 provider oracle. Material caveat: the retained grid test's executable assertions were not weakened by unwrapping its branch. Source correction to prior revision: the exact empty-provider contract is terminal.js:839-846, not SPEC_impl.md:1810-1838 (which includes unrelated Shift+Enter lines); the specification's clipboard paragraph begins1823. No prohibited coverage sources searched and no tests or reproductions run.

Returned proposal (claims-109, event 15): **retain-verified**, suggested bucket **other**.

The old browser provider's exact-empty OSC52 response assertion disappears while adopted old terminals remain served. The grid's no-page-report oracle exercises a different security boundary. This is an intermediate test gap; it does not establish disclosure.

Original stronger consequence: Initial line 2 proposed dropping the test rewrite because the grid assertions survived and no functional leak was established; lines 14-15 explicitly withdraw that rationale. A real clipboard leak would be security-impacting, but no leak is claimed. Line 15 corrects the provider reference to terminal.js:839-846 and the historical spec paragraph to :1823 onward.

Unresolved premise or limitation: No shipped clipboard disclosure route or runtime failure was shown. The provider no longer exists at tip, so this is historical coverage loss, not a request to restore it.

Proposed basis: Retain the intermediate provider-test gap; keep other for established maintainer-test harm. Any parent decision to dismiss it, including because retirement later removed the provider, should preserve and independently check the security-related earlier proposed drop.

Existing parent disposition (event 15): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g174 record](records/review-through-00de0b6c/groups/g174.json). Record ID: `c2`.

