# Birth-time oracle treats execution errors as absent capability

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Birth-time oracle treats execution errors as absent capability.

## Details

`F17 / COR-BIRTH-ORACLE` — **definite** — `crates/farhelm-supervisor/src/working_copies.rs:2072` — Birth-time oracle
treats execution errors as absent capability

Four checkout-ownership tests can report a skip when their independent filesystem check failed, hiding the production
regression that check exists to catch. When production records no directory creation time, the tests ask an external
`stat` command whether the filesystem actually supports one. If it does, missing production data should fail the test
rather than excuse it.

The helper returns only a Boolean. A missing command, unsuccessful exit, invalid UTF-8 output, or unparsable result all
become `false`, exactly like a successful answer saying creation time is unavailable. Each affected test accepts that
`false`, prints `SKIPPED`, and returns successfully. Thus missing production creation-time data combined with a failed
independent check becomes a passing test. The command uses GNU `stat -c %W`; the default BSD `stat` on macOS does not
support that invocation, so the supposed independent safeguard is also platform-dependent.

Make the probe platform-aware and return a result that distinguishes supported creation times, explicitly unavailable
creation times, and probe errors. Fail on execution, decoding, or parsing errors. Skip only after a successful
observation explicitly establishes that creation time is unavailable.

Suggested bucket: other

Possible cover: none

Caveats: No production creation-time failure or macOS execution was observed during this review. The runtime_data
pass-two crosscheck supports the finding. The GNU/BSD difference matters when production reports no creation time; it
does not imply these tests always skip on macOS.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `runtime_general p2`.

Possible cover recorded during collection: none identified.

Collection caveats: No production failure or mac execution; runtime_data p2 crosscheck supports.

## Filed reviewer metadata

- `runtime_general p2`: confidence as filed: **definite / confirmed** for error misclassification. Suggested bucket as
  filed: **other**
