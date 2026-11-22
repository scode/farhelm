# Shell teardown coverage stops at the signal and Delete reply

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

The tests can pass without showing that a command typed in Bash or Zsh is saved when Farhelm stops the session.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/sweep.rs:3553`.

tip sweep.rs:3553-3587 calls the reaper directly with a fake manager and a marked Rust sleeper. The new case asserts the
fake scoped path's success and the survivor fixture's SIGHUP exit. The later real-Bash test, tip
crates/farhelm/tests/e2e/host_creates.rs:224-243, checks startup, evaluated input, and a successful Delete but never
checks the fixture's history. The searched supervisor and integration assertions contained no saved-history check.
SPEC.md:2074-2075 promises an opportunity to save history through hangup. A main-shell lifecycle test with isolated,
enabled history and a unique evaluated line could check the intended result through public Stop or Delete. Signal-level
assertions remain useful coverage for scoped and portable routing. Bucket: other, coverage weakness only. Confidence:
source-supported assertion boundaries. Actual Bash/Zsh history behavior unobserved. Possible coverage: lifecycle_e2e,
lifecycle_unit.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/l6`.
