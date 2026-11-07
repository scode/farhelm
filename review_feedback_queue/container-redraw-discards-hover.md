# Container redraws discard unchanged file hover checks

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A container terminal can lose a completed file hover when other cells on that row redraw. One redraw can cause temporary
rechecking. Another relevant row change during replacement admission can leave the link absent while the pointer stays
at rest; movement or completion of a later gesture can retry. A first click started without a hovered link does not
activate the file.

## Details

Source coordinates from the original records: `crates/farhelm-ui/assets/terminal-files.js:393-410`;
`crates/farhelm-ui/assets/grid-terminal.js:1347-1435`. References below use these paths' basenames unless another path
or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Original causal references and timing conditions remain preserved in earlier l1 events: FileLinks reclaims only before
its zero-delay departure clear (terminal-files.js:393-410,458-480); container admission waits for host stat (301-330;
terminal.js:1353-1380); a second dependent-row replacement invalidates the callback and no null-hover paint retry runs
(grid-terminal.js:1347-1350,1429-1436). This is conditional repeated-row interference, not general permanent click loss.

Peer g023/l4 independently corroborates this source ordering and adds the precise recovery qualification:
grid-terminal.js:1788-1794 retries on movement, and :1674-1676 also retries after a later gesture release. Its
:1752-1753 and :1667-1671 require an existing hovered link at press and matching hover at release, so the first
unhovered click cannot activate the former file. Peer also confirms answerLines includes the hovered row
(grid-buffer.js:477-484), whole decoded rows are fresh objects (grid-model.js:224-252,560-562), and existing grid tests
separate synchronous redraw reuse from container admission. No unresolved incoming interface question remains. This
revision narrows the prior wording 'until pointer moves' to 'while at rest; movement or gesture release can retry',
rather than treating movement as the sole recovery path.

SPEC.md, Terminal experience, :2673-2674 explicitly says a redraw beneath a resting pointer on the same path
text/target/position is not a new hover; introducing spec :2663-2664 says the same. Suggested bucket: other, unchanged.
Confidence: confirmed conditional source route, corroborated by peer; no runtime experiment. proposed_drop:false.
possible_cover:g023/l4 is the same cross-group issue; consolidate, not a second finding. No security/data-loss
consequence was established or lowered. Preserve all prior records. Discovery pass has not yet started beyond reviewing
incoming known evidence.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g022/l1`, `g023/l4`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
