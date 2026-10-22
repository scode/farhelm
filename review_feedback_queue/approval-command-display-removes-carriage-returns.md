# Approval command display removes meaningful carriage returns

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

An approval card can show a different command from the one it authorizes. The display removes carriage returns in CRLF
pairs while the launch retains them. In a shell script argument, that difference can change which operations run.
Execution and browser rendering have not been reproduced.

## Details

Command and wrapper rows use `PeerBlock` (`crates/farhelm-ui/src/approvals.rs:525–532`). Its body removes the full CRLF
terminator and restores only LF before rendering (`peer.rs:355–370,415–417`). A bare CR is escaped, but a CR immediately
before LF disappears. This is a semantic transformation when the text represents executable arguments: a backslash
before LF is a shell line continuation, whereas a backslash before CR leaves the following LF as a line boundary.

Direct command admission retains the text (`farhelm-helm/src/agent_launch.rs:233–255`, `sessions.rs:2265–2277`). Command
validation shell-splits it and checks its argument shape without refusing CR
(`farhelm-proto/src/session_launch.rs:140–173, 425–430,461–486`). A quoted script argument can therefore retain CRLF.
Supervisor argv validation refuses NUL, not CR (`farhelm-supervisor/src/agent_kind/mod.rs:2305–2320`), and the child
receives the retained arguments (`farhelm-supervisor/src/launch.rs:1161–1163`).

Wrapper vectors also admit CRLF (`farhelm-proto/src/launch.rs:605–620`). The agent-supplied wrapper row and the full
launch-command row both pass through the same display transformation. This includes the direct Allow required for an
agent-supplied wrapper on a guarded destination, and template-wrapper write approval. A faithful second command display
does not repair this case. New template command/resume fields and initial prompts separately reject CR
(`farhelm-proto/src/launcher.rs:398–407,444–498`); those admission paths are excluded.

Keep approval command text faithful to the retained arguments, or visibly disclose significant control bytes without
normalizing them away. Inspect direct command, agent-wrapper and template-wrapper callers when choosing the boundary. No
runtime verification is requested or authorized by this review; the user stopped reproduction work.

Suggested bucket: **provisional highest**, for the conditional approval/execution mismatch. Confidence: **confirmed
static admission/rendering mismatch; likely consequential execution difference**, with shell/parser/browser behavior
unverified. The user must approve; direct command launches still require the destination's YOLO allowance. No actual
unauthorized operation, credential exposure, or lost work was observed. Preserve these limits in triage.

Possible cover: `SPEC.md:2564–2586` and `SPEC_impl.md:5626–5664` require the shown action to match the approved action.
The post-clone editor decision (`TRIAGE_OUTCOMES.md:13481–13499`) and template command-save decisions (`8437–8472`)
cover different admission/rendering paths. No exact Planned, BUGS, queue, ledger or filter coverage was found. Proposed
drop: none.

Provenance: persistent reviewer `A21-card-meaning/A21-F2`; parent checked the rendering, admission, execution and
coverage sources at the reviewed revision. Source inspection only.
