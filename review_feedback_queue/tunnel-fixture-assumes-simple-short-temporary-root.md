# Tunnel fixture assumes simple short temporary root

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

The tunnel transport tests can reject correct behavior when the temporary root needs path escaping or is too long for
SSH sharing.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/transport.rs:1079-1087,1108-1136 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g047/l4, event 24):

Reviewed commit 88bf83e267b5066815448885c61ed4148b851930; unchanged assertions/fixture persist at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b:1081-1089,1110-1138. TLDR: Tensorlake transport unit tests can reject correct
production argv and sharing fallback when the runner's temporary root needs escaping or is too long for SSH sharing.
Impact is test/development portability, not live product behavior. The new a_tensorlake_row_is_dialed_with_farhelms_key
obtains state from fake_ssh's tempfile::tempdir (original transport.rs:842-849), then compares printed argv with
IdentityFile around RAW path text (1079-1087). Actual tensorlake/ssh.rs:95 uses ssh_config_path_option, whose encoder
ssh.rs:612-615 doubles percent and escapes backslashes/double quotes. For a short ambient temp root with one percent,
actual IdentityFile has double percent and expected raw substring has single percent, so assertion fails without
invoking any real SSH client. The new release test likewise compares raw state ControlPath against encoded argv
(1127-1136); its script additionally inserts raw log.path inside shell single quotes (1111-1115), so a single-quote path
breaks intended log redirection/fixture parsing. It always reads the log and expects two exits (1120-1126), but
ssh_control_exit_args:406-414 intentionally returns None if control_socket cannot fit; control_socket_within:579-591
bases this on ambient full state-path length and each purpose's suffix. A long valid temp root therefore creates no
release log or only one run, causing an unwrap/count failure for correct behavior. Exact dependency premise settled:
Cargo.lock:6135-6138 pins tempfile 3.27.0. Cached source src/dir/mod.rs:64-65,218-219 delegates tempdir to TempDir::new
to Builder::tempdir; src/lib.rs:595-596 chooses tempdir_in(env::temp_dir()); src/env.rs:38-50 delegates to
std::env::temp_dir unless a caller explicitly set the library override, and does not sanitize path characters/length.
The test has no override or fixture-specific root. SPEC_impl.md Helm-supervisor transport:3028-3043 explicitly requires
long paths to turn sharing off rather than fail; the test-authoring checklist requires establishing the premise before
asserting behavior. Confidence confirmed source-level fixture/expectation mismatch, no runtime test or
process-environment mutation performed. Suggested bucket other; proposed_drop false; possible_cover unknown, parent
checks coverage. Fix by controlling a short sharing-capable owned fixture for the master-release test, properly
shell-quoting the log path, and asserting the encoded-path contract without treating raw filesystem paths as argv
syntax. Do not remove production escaping to make these tests pass, or mutate the test process environment. Temporal:
ambient fake_ssh predates this commit; the raw encoded-value expectations, raw script path interpolation and
unconditional sharing expectation are NEW in these assigned tests. This is distinct from known g046:f10/f11 real
old-client tokenizer/pinning defects: here a fake argv printer suffices, with no OpenSSH 7.6 premise. No source
uncertainty or open lead remains; static verification only, not a recorded runtime failure.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g047/l4`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
