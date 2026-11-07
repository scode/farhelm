# An unmounted attempt bypasses Restart's hold

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A terminal still waiting for fonts/helpers can construct the old component and open a socket while Restart tells it to
wait. The established l1 consequence remains unnecessary component/socket work; successful grid adoption leaves no
established lasting defect. A separate known pre-notice input-format hazard must not be treated as safe by this limited
conclusion.

## Details

Source coordinates from the original records: `crates/farhelm-ui/src/session_view.rs:1682`;
`crates/farhelm-ui/assets/terminal.js:3226-3228,3287,3593-3637`; `crates/farhelm-ui/assets/terminal.js:3226-3229,3287`;
`crates/farhelm-ui/assets/terminal.js:3226-3228,3287,3593-3635`;
`crates/farhelm-ui/assets/terminal.js:3226-3228,3284-3287,3593-3635`. References below use these paths' basenames unless
another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Pinned tip 407a49435278844248d573083fdea6e23fd96c02. The existing source sequence and its refinements are retained in
prior l1 records: sync:3226-3229 handles pending-to-known only and skips a pre-existing kindPending=false attempt
at3287; readiness mounts its captured tmux spec at3593-3635; SessionView begins Restart without changing gen until
completion (:1365-1380) and supplies pending via1682-1686,3578-3588. Mounted-island retention remains distinct and
intentional. SPEC_impl.md, The grid client, requires pending attempts to mount nothing until the kind is known;
terminal.js:3582-3583 says `mounts nothing at all until a sync names the kind`. The bypass violates that contract. The
component starts hidden; server admission is not guaranteed by opening the WebSocket. Successful adoptGrid at6157-6201
replaces drivers/records on the same socket, without an established lasting display/input defect. Suggested bucket for
this supported l1 consequence remains other; no measured resource cost.

New authorized interface synthesis, not discovery: read g014/l3's refined corroboration and g015/f6's source-confirmed
duplicate; neither finds an additional client fence or lasting post-adoption failure. Read exact latest g015/f8. That
separate record reports a source-traced conditional raw-xterm-to-structured-grid input mismatch before a delayed first
grid notice: rejected ordinary typing and, separately, literal pasted JSON escapes being decoded as a submit key, with
potential unintended command submission that could modify/delete user-owned work. The delayed-notice interactive
interval is unobserved after investigation; the whole framing route exists at the exclusive base. Preserve f8's
potential highest consequence and investigated uncertainty. The commissioner's proposed pre-base scope exclusion is not
a safety refutation and requires independent fresh-context handling for the potential highest branch. No payload was
constructed or experiment performed here.

The earlier l1 statements about no established security/work-loss route are limited to the l1-specific sequence and
successful post-adoption behavior; they do not assert that raw pre-notice compatibility input is safe. No joint
l1-specific delayed-notice occurrence or additional lasting consequence is established in this record, so l1 is not
escalated by assumption or duplicated as a new hazard. Confidence: confirmed client hold bypass; f8's conditional
transformation is reported from its peer evidence, not independently reproduced. proposed_drop: false. possible_cover:
duplicate g014/l3 and g015/f6; related known pre-base hazard g015/f8 remains separately owned, not a basis to drop or
safety-refute either consequence. No forbidden coverage sources checked. c1's original hypothetical security
premise/source refutation and c2/c3/c4/l3 dispositions remain preserved.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g017/l1`, `g014/l3`,
`g015/f6`, `g016/l2`, `g018/l2`. The retained append-only histories preserve all earlier claims and corrections.
Reviewed source references name their own revisions; the inclusive reviewed tip is the hash above. This review used
source inspection only, with no product runtime experiments or live-install changes.
