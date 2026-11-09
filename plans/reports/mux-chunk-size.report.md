# Smaller transfer chunks on the shared host connection

## What this was about

Uploads, downloads and tunnels share each host's connection with terminal input and output. The maintainer chose 32 KiB
transfer chunks instead of 256 KiB, keeping the existing transfer window and terminal priority. This reduces the bulk
payload that can queue ahead of typing from about 1 MiB to 128 KiB. It is a bound on queued data, not a measured latency
improvement or a bound on bytes already in the network buffers.

All three transfer paths use the smaller chunks. Their queues continue to hold the same approximate byte window,
downloads still fill chunks across short reads and send a partial final chunk, and the transport explanation now records
the queues, credit, acknowledgements, stall limits and intentional priority.

## Things you should know

Remote supervisors need updating. Peers using the former chunk size cannot safely transfer with this version: their
outgoing chunks exceed the new limit, and their smaller file-transfer queues cannot hold a window of the new frames. The
protocol-version check refuses that mismatch before transfers begin.

Terminal output from a host still intentionally takes precedence over downloads and tunnel traffic toward the helm.
Those transfers can slow or stall while terminal output stays heavy. Uploads and traffic toward the host are not held
behind that output. The 4 MiB transfer window, 8 MiB frame ceiling, terminal sizes and scheduling are unchanged.

## Open questions and possible follow-ups

None required for this plan. Automatic window sizing and a cap on the Mac app's own tunnels remain separate TODO
entries. No measurements or fairness changes were added, following the maintainer's decision.

## The PRs

- [#3254](https://github.com/scode/farhelm/pull/3254/changes) — smaller transfer chunks and the transport contract.

The PR remains a draft for the landing monitor.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E <protocol and transfer selection>`
  passed 318 tests; recorder run `c6ed3553-071c-4019-b7c7-d57a80f89a1c`. This covers the protocol pins and mismatch
  refusal, upload rechunking and credit, control traffic during a bulk burst, short-read downloads, relay byte credit,
  half-close and transfers beyond one window.
- `cargo nextest run -p farhelm-helm -p farhelm --lib --test e2e -E <nested relay and upload/download/relay selection>`
  passed 58 tests plus the required fixture-build setup; recorder run `d2caf92c-65d9-4763-b7bf-d7a481f193eb`. Actual
  upload, download and bidirectional tunnel transfers passed, including a partial final download, over-window data,
  oversized-chunk refusal and upload acknowledgements ahead of terminal output.
- Both commands used pinned nextest 0.9.148 and tmux 3.7c, four slots and zero retries. Both reports and retained output
  are complete, with no runtime `SKIPPED`, dropped console output or forced cleanup. The first run's asserted
  should-panic test produced its expected panic. There were no failed runtime attempts. There are 376 actual passing
  tests; the portable summary's 377 reported passes include the fixture setup. The 5,502 selected-out tests are not
  counted as passes.
- `cargo clippy -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --all-targets -- -D warnings`, Rust formatting,
  changed Markdown formatting, changelog format and whitespace checks passed. The isolated delay checker inspected 430
  delays with no missing rationale. The instruction-link checker passed with zero errors and one pre-existing
  missing-heading warning in the triage history.
- Both runtime commands captured base `b0893258` with source fingerprint
  `b9a9b0fe92cd6f64b8f543a82f06134fd8daa59c0963a99b59e511b1bb542eec`. Only acknowledgement prose changed afterwards.
  Rebase onto `b80071ca` added queue/report bookkeeping, leaving the feature diff unchanged; the pushed commit
  `2c311184` preserves the reviewed feature diff. Each runtime result and Clippy remain applicable; formatting and
  whitespace were checked again after the rebase.
- Skipped the full workspace, browser, native desktop, installer, provisioning and hosted CI runs: no UI, installation
  or scheduling behavior changed, and the targeted tests cover the affected wire and transfer paths. No native macOS or
  throughput/latency measurement claim is made. The maintainer's live installation was not touched.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol/high reviewer found no outstanding issues. The initial transport prose
overstated when download credit advances; it now distinguishes upload disk writes, relay socket writes and the helm
taking download bytes from its receive queue. The same reviewer verified that correction against the source. No finding
was rejected or required behavior omitted. The documentation pass and commit/PR wording cold read are complete.
