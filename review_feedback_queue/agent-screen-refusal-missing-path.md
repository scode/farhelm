# The capture tool's invalid-path error omits the path

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

If a capture working directory contains bytes that are not UTF-8, the new backend refuses it without displaying the path
it refused.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/agent_screen.rs:158-161`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Introduced by 2d5859c5f144e0c286a0576976aae2d1c66ddc7c: git log --diff-filter=A identifies agent_screen.rs creation
there; its :154-178 contains the same refusal, and the introduction-to-tip diff for agent_screen.rs and
agent_screen_host.py is empty. The governing requirement already existed there at SPEC.md:3997-4003 and remains at tip
:4001-4007 under 'Paths that are not valid UTF-8'. Exact applicable text at tip: 'Every surface that meets one refuses
it with a clear message naming the path as well as it can be shown'. The following sentence permits replacement
characters in display but forbids acting on replacement text.

agent_screen.rs:158-161 rejects spec.cwd.to_str() with only 'the working directory is not UTF-8', omitting the PathBuf
value entirely. main.rs:1973-1992 passes the absolute PathBuf and propagates this result. PathBuf admission does not
preempt it: pinned Cargo.lock selects clap_builder 4.6.7; that dependency's builder/value_parser.rs:1004-1031 accepts a
nonempty OsString as PathBuf without UTF-8 validation. The supported Python caller at agent_screen_host.py:108-109
passes cwd.resolve() into --cwd; :123-132 incorporates the backend's generic log into HostError but does not add cwd.
This is a source-established omission in the refused-path diagnostic, not an experiment showing an actual failed
capture. Include the display/debug form of spec.cwd in the error context so the caller sees the rejected path. An
injected non-UTF-8 PathBuf validation test can verify the diagnostic without launching any real agent.

Narrowed original concern, preserved explicitly: no wrong-path or silent replacement execution was found.
hosts/spawn.rs:508-525 encodes argv/cwd/env as native bytes; :335-352 passes the socket natively;
terminal-host/src/spawn.rs:32-41 keeps its native PathBuf. The cwd is refused before host spawn. Supported scripts mint
ASCII run directories. Thus no claim of user-work loss, credential exposure, unsupported run-dir execution, or agent
survival arises from this finding. Suggested bucket other; confidence confirmed source-level omitted-path diagnostic,
with no runtime experiment. proposed_drop false for the diagnostic; original corruption hypothesis proposed excluded as
source-refuted by byte transport plus early refusal. possible_cover none in read peer records. Cross-group primary
source belongs to g047 capture backend; g012 records this under its own ID because the assigned CLI exposes the failing
validation boundary. Do not conflate it with g047/d1 startup handle-transfer failure, g047/d3 executable selection, or
l3 failed-stop recovery loss.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g012/l8`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
