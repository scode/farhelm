# Grid argv observer can concatenate two versions of one row

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The structured-launch test can decode arguments the fake process never received if attachment catches a wrapped hex
witness partway through a continuation row. That can reject a correct launch. The event-shape defect is supported by
source; the exact scheduling cut remains unverified after investigation.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/structured_launches.rs:365-366`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **other**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

The fixed current explicit Codex case suffices to exceed WIDE_COLS=1000 without invented user input.
structured_launches.rs:759-767 selects model release/candidate'42;$literal (29 bytes), high effort and YOLO.
invocation_with_script at 69-164 adds the fake-agent, script and home arguments plus -m, model, -c,
model_reasoning_effort=high and --yolo. fake_agent_launch at 273-305 retains them and appends {farhelm_args};
core.rs:3144-3198 expands that placeholder through CodexIntegration. agent_kind/mod.rs:1325-1402 adds --no-daemon,
hook-trust/feature flags, and SessionStart/Stop hook commands. Defaults enable hooks and announce instructions
(core.rs:1341-1358; agent_kind/mod.rs:2622-2637). The hook command is the shell-quoted executable plus ' internal hook
--vendor codex' (798-826), TOML/JSON quoted. teststate/lib.rs:70-93,708,800-817 yields the ASCII home
/tmp/fh-it.<six>/d, 19 bytes.

Static byte count for an ordinary ASCII hook executable of length L, with no single quote/backslash:
fake-agent/script/home arguments contribute 68 NUL-terminated bytes; --no-daemon contributes 12; trust/feature arguments
and two hook overrides contribute 266+2L; explicit model/effort/YOLO contribute 71. Total argv payload is 417+2L bytes.
The wrapper at structured_launches.rs:223-232 prints its 27-character prefix followed by two hex characters per payload
byte: 861+4L columns. It wraps for L>=35. For the ordinary checkout-relative target/debug/farhelm path in this checkout
(L=56), that is 1085 columns. This is source/layout arithmetic, not execution or a claim to have read a current
fixture's resolved Cargo binary path. Spaces, quotes or longer paths can only increase the relevant encoded length.

The continuation duplication is now checked through the complete transformation. grid_stream.rs:423-460 snapshots the
current screen; grid_output.rs:403-450 later sends complete contents of only the rows that differ, and stores the new
screen. Once the first hex row is full and wrapped, it need not change as the second row fills. An initial snapshot
holding that first row and a partial second row followed by a live replacement of only the second row is therefore a
legal source-supported shape. harness.rs:151-200 discards row coordinates and appends both versions. normalize_pane_text
at 2893-2928 strips escapes and trailing blanks, but does not apply rows or remove duplicates.
structured_launches.rs:508-542 finds the unchanged old prefix and concatenates the partial second row and the complete
replacement row, filtering blank snapshot rows along the way. When later argv/READY rows satisfy 495-497, an even-length
duplicated hex fragment can produce wrong argv while preserving the trailing NUL; other cuts fail decoding. A changed
prefix row would reset rfind, but this shape expressly leaves that full row unchanged.

Exact remaining unverified-after-investigation premise: attachment's initial screen read, or successive live screen
reads, must actually occur while the fixed witness's second row is partial, with a subsequent view filling it and
leaving the first row unchanged. The shell od pipeline and host byte reads do not establish that scheduling cut for a
retained current run. No trace was available and runtime experiments were forbidden. A deterministic grid fixture with a
repeated row coordinate, or a retained real grid trace showing that cut, would settle occurrence. The source-supported
width calculation and transform mismatch no longer depend on that missing evidence; only actual scheduling/frequency
does. A coordinate-aware screen/history observer is the direct repair.

Suggested bucket: other (maintainer-test false failure or corrupted witness only). Confidence: likely, with the
scheduling premise named above. proposed_drop false. possible_cover none.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g045/g045-l3`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
