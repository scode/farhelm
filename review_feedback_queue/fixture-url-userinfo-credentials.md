# Configured URL credentials bypass fixture privacy guards

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The capture tool can accept and write a screen containing proxy credentials in a configured local URL. Those bytes can
reach the fixture tree intended for public commits. This review has not observed an actual credential-bearing capture or
public credential leak.

## Details

Source coordinates from the original records: `scripts/capture-agent-screens.py:98-100,273-285,729-732`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **highest**. Source-confirmed conditional behavior does not imply
an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the
finding.

Later-turn synthesis preserves the initial source-established configurable/displayed URL route. Versioned openai/codex
rust-v0.166.0 core/src/config/mod.rs:4035-4053 carries nonempty cfg.openai_base_url into the selected provider;
model-provider-info/src/lib.rs:567-570,745-749 preserves that string; tui/src/chatwidget/model_popups.rs:42-79 directly
emits its trimmed value in the model menu without credential redaction. Primary source links retained:
https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/core/src/config/mod.rs#L4035-L4053 and
https://github.com/openai/codex/blob/rust-v0.166.0/codex-rs/tui/src/chatwidget/model_popups.rs#L42-L79. The pinned Codex
unknown-model-picker.txt:29 already demonstrates this warning location with an ordinary scrubbed endpoint. No new vendor
experiment performed.

Checked the earlier/later boundaries this turn. agent_screen.rs:237-251 reads emulator.screen_text and title and returns
them directly; agent_screen_host.py:160-164 reads those JSON fields without redaction. drive_codex:605-608 saves the
captured model menu. Recorder.save:273-285 runs the only Scrubber replacements/check before writing staging files. For
http://captureacct:proxysecret@localhost:8080/backend-api/codex, where account/secret are unrelated to known local
identity literals, Python LOCAL_URL:98-100 fails because userinfo precedes the host; EMAIL:93 sees no dotted domain.
Rust contains_local_url:214-234 treats captureacct as host, and contains_email:189-203 sees no dotted domain, so the
independent guard also accepts the text. http://captureacct:proxysecret@[::1]:8080/ has the same bypass. Dotted IPv4
userinfo is NOT claimed to bypass both: Rust's broader email detector flags that form, as corrected in the earlier
revision.

No inspected earlier redaction or enforced publication gate prevents these userinfo bytes from being saved.
capture_harness:729-732 copies staging files into the checkout before run_fixture_tests is called at main:799-801;
--no-test is also allowed. The later test checks the same inadequate guards (screen_fixtures.rs:291-313), and even a
test failure would not roll the writes back. docs/agent-screen-fixtures.md:29-31 tells the maintainer to commit the new
version directory; 54-60 promises replace/refuse plus an independent check and explicitly requires a manual diff read.
That human review can catch the bytes before publication, and the script itself neither commits nor pushes. Therefore
the established result is an automatic acceptance/writing gap and a supported disclosure route upon later publication,
NOT evidence that anyone published real credentials or that all manual/publication controls were bypassed. Source search
limited to scripts/.github/docs fixture references found no additional automated capture privacy gate; no unrelated
queue/publication-history investigation was performed.

Suggested bucket highest retained for potential public disclosure of reusable credentials through the documented
workflow; the conditional publication premise is explicit and no actual leak is asserted. Confidence confirmed
source-level acceptance/writing route; public disclosure remains conditional on later committing/pushing without a human
catching it. proposed_drop false. possible_cover none. If the commissioner views this conditional consequence as not
crossing the established-route priority rule, that priority decision must be surfaced and recorded rather than silently
treated as an observed leak or lowered. Fix: redact userinfo before authority classification and independently refuse
credential-bearing URLs, covering localhost and bracketed IPv6 examples. SPEC_impl.md Testing accepts local
process-argument secrets and hostile-local-account risks, not writing credentials into accepted public fixtures.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g048/f2`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
