# Terminal synchronization can take focus from a header control

Reviewed commit: 4ba9bf48d9f42c1ab4218ff56dd3806b04bc47fb

## TLDR

Delayed terminal-script loading and automatic fallback after a tab disappears can move focus from a surviving header
button into the agent terminal. The source paths are established; harmful commands or approvals are not.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`crates/farhelm-ui/assets/terminal.js:3101-3106,5210-5222,5253-5257; crates/farhelm-ui/src/session_view.rs:1822-1823,1871,2023`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Final cross-group synthesis of g141/f3 at product commit 4ba9bf48d9f42c1ab4218ff56dd3806b04bc47fb; pinned tip
463cac1528b5c5e259404975380d321a3fff0412 retains direct fallback focus at terminal.js:2896-2901 and selection derivation
at session_view.rs:1578-1589,1637,1788. Verdict: supported additional conditional source mechanism under the same focus
record, not expressly accepted automatic navigation. Selected shell removal derives agent focus; sync directly focuses
the existing island unless a listed keyboard owner vetoes it. A surviving plain header button is not a veto. This does
not call takesFocus and therefore does not depend on the delayed-script first-sync overwrite or replay. The
already-revealed, available agent premise distinguishes it from a hidden/unready terminal whose focus attempt would be
inert. Normal future xterm onData is encoded and sent on the open island websocket (5210-5222,5253-5257); this
establishes the generic input route, not a destructive command, tool approval, unobserved prompt default or loss of
work. OTHER for both branches; g141 high is unsupported by focus displacement alone. Confidence confirmed conditional
source mechanism, runtime frequency/focus execution unmeasured. Six pinned coverage sources searched. SPEC.md:1396-1416
accepts silent tab reaping and discarded tab scrollback, not expressly taking focus from surviving controls;
SPEC.md:1504-1509 accepts competing mixed-method user navigation within tens of ms, a different trigger.
SPEC_impl.md:572-577,614-624 and ledger:15933-15951 protect questions/dialogs during fallback, not acceptance of
plain-button displacement. Reveal-only rules at SPEC_impl.md:1100-1104 are not misrepresented as a direct-focus
contract. Related queue items concern launcher-answer removal or bell handbacks and have different
triggers/consequences. FILTER subsecond race needs a person to win a short window; this trigger permits focusing the
button well before independent shell disappearance. No whole-finding rare-glitch filter match established. Report
732f3a8a90ba1a96926509e89012463df3bb12bc:65-68 discloses the branch but is provenance, not express acceptance.
possible_cover none exact; proposed_drop false. Preserve original c1/c2 and post-sync g210/f2 repair exclusions
unchanged. No new highest route established; no tests/reproduction performed.

Provenance: `a066/focus-initial-sync`, `g141/f3`, `g209/f2`, `g210/f2`, `g279/f2`, `g279/f5`. These are reviewer-local
identities, not additional findings. Proposed drop: none for the retained remainder.
