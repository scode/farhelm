# Muse session tracking

## What this was about

Muse could run in a Farhelm session, but Restart could not resume its conversation. This work adds conversation resume
through resident wrappers and reads Muse's working, idle and directory-trust screens. The original scope also included
questions and approvals during a conversation. The 2026-10-29 Decision accepts the smaller scope and defers that
recognition; the original TODO now names only the deferred work. The separate TODO recording that scope judgment
remains.

## Things you should know

Muse 1.4.14 is the minimum verified version. Farhelm supplies hooks for each launch without installing a plugin or
changing Muse's configuration. Real private runs with the offline echo provider verified the conversation identifier,
start and end-of-turn reports, resume, and the start hook's instructions announcement. Muse uses the same positional
attribution and shared conversation pin as Claude, including correction when a parent reports after its child.

Working, Thinking, the resting composer and directory trust are covered by captured Muse screens. General conversation
questions and approval dialogs remain unverified and unrecognized: **a Muse session can show Idle while Muse waits for
an answer**. This is now explicit in SPEC's Status section and on the Muse documentation page. No supported way to make
echo ask a real question was found. The bounded custom-provider probe stopped before a question at an undocumented
provider response; that is a test-setup limitation, not proof that Muse cannot show recognizable questions. No operator
credentials or saved configuration were copied or changed, and no further provider reverse engineering was done.

Existing stored Muse launches gain the new integration on their next start, but retain their absent resume command.
Recreate those sessions through Clone or Replace with to gain resumable launches; there is no migration or backfill. The
new Muse agent kind bumps the peer protocol to 65, so update the helm and supervisors together. The support policy now
allows two weeks after a newer agent release before dropping the older version, without extra compatibility code.

The changed documentation pages are Muse, Agent wrappers, the agents support matrix and Agent hook injection. No preview
server was started during unattended execution. The four PRs remain drafts for the landing monitor.

## Open questions and possible follow-ups

Recognizing conversation questions and approval dialogs requires an authenticated test provider or a documented provider
fixture protocol that can make Muse ask a real question, followed by real screen captures. That work remains in TODO; it
is outside the approved delivery scope. The maintainer can also revisit the smaller-scope decision through its separate
near-term TODO entry.

## The PRs

- [#2785 — agent version support policy](https://github.com/scode/farhelm/pull/2785/changes).
- [#2786 — Muse's own integration kind](https://github.com/scode/farhelm/pull/2786/changes).
- [#2788 — Muse conversation resume](https://github.com/scode/farhelm/pull/2788/changes).
- [#2789 — Muse working, idle and directory-trust status](https://github.com/scode/farhelm/pull/2789/changes).

## Checks run, reused and skipped

This completion round changed only SPEC wording and the deferred TODO. `dprint check SPEC.md TODO.md` passed after
formatting. `python3 scripts/check-agents-md.py` passed with zero errors and two existing warnings in historical
citations. `python3 releasing/check-changelog.py format` passed. A recorded
`cd website && bun install --frozen-lockfile && bun run build` passed, including internal links and anchors, run
`acada2f8-c1e8-424c-98ad-e5ef25f4b717`. It built 32 pages using Node 26.12.1 and Bun 1.4.2; existing empty i18n/404
content warnings did not prevent the build.

The following focused runtime evidence is reused from the implementation round. All commands ran through the recorder on
Linux x86_64 with nextest 0.9.148, pinned tmux 3.7c, four slots and zero retries. Their complete reports and retained
output were inspected; selected-out cases are not counted as runtime passes. The rebase changed none of the four
commits' patches; upstream added only queue/TODO bookkeeping, and this round changed no executable or fixture bytes.
Each recorded dirty source tree below identifies the actual tested input rather than claiming its base HEAD was the
finished implementation.

- Kind integration: `362f4ae5-2ad9-449a-ae6b-84cfb84ee2d0`, 199 passes, 3448 selected out. Command:
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --lib -E 'package(farhelm-proto) | test(/muse|structured_launches_round_trip_with_their_matching_kind|new_agent_kinds_round_trip_and_unknown_spellings_fail_decode|kinds_without_a_dedicated_reader|session_agent_kind_decodes_tolerantly|every_agent_type_composes|explicit_choices|a_custom_model/)'`.
  Tested the dirty kind implementation above `1ccf3909`, source fingerprint beginning `1551068609a65d97`. Reuse is
  confined to unaffected coverage; the historical conversion and store correction use the following two runs.
- Corrected historical conversion: `9b274ef6-44a5-45b4-9887-9c2314b1a53f`, 6 passes, 181 selected out, command
  `cargo nextest run -p farhelm-proto --lib -E 'test(session_launch::tests::)'`; dirty source above `1ccf3909`,
  fingerprint beginning `82644fc8fab831c1`.
- Corrected Muse storage fixture: `9900b030-a3d2-4e47-b5da-add20e65c271`, 1 pass, 1339 selected out, command
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(=store::tests::structured_launches_round_trip_with_their_matching_kind)'`;
  dirty source above `1ccf3909`, fingerprint beginning `cc70cd02dbe6a6bf`.
- Capture, hooks, launch compilation and shared Claude regressions: `f38327a9-c5d7-494b-af5d-56608bc13689`, 158 runtime
  passes across six binaries, 3309 selected out. JUnit additionally counts one setup case. Command:
  `cargo nextest run -p farhelm-supervisor -p farhelm-proto -p farhelm-helm -p farhelm -p farhelm-fixtures --lib --bins --test e2e -E 'test(agent_kind::tests::) | test(agent_kind::muse::) | test(procs::tests::claude_) | test(procs::tests::muse_) | test(service::core::tests::claude_pin::) | test(hook_report::tests::) | test(service::notifications::tests::) | test(launches::tests::) | (package(farhelm-proto) & test(/every_reporting_kind|session_launch::tests::/)) | test(hook::tests::) | test(muse_hook_fixture_preserves_resume_and_injected_hooks) | test(/a_two_level_resident_wrapper_captures_and_resumes_(claude|muse)|a_shelled_out_(muse_)?child_cannot_replace_the_foreground_report|child_first_(claude|muse)_capture_is_corrected_by_stop_and_resumes_parent/)'`.
  Dirty capture source above `e9130741`, fingerprint beginning `19d370fa85bc2096`, subsequently committed as `f541585f`.
  All six Claude/Muse wrapper, rival-report and ancestor-correction end-to-end witnesses passed; their retained output
  had no runtime substrate-skip marker.
- Screen interpretation and captured fixtures: `0b9cb84a-fca5-4050-9d0d-cfc1006e3750`, 24 passes, 1330 selected out,
  command
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(agent_kind::screen_reader::tests::) | test(agent_kind::screen_fixtures::)'`.
  Dirty status source above `f541585f`, fingerprint beginning `18ede09408e6d222`, subsequently committed as `f1f599aa`
  with a prose-only SPEC correction. No runtime substrate-skip marker was present.

The prior round also passed all-target Clippy for proto, supervisor, helm, CLI, fixtures and UI, shipped CLI-bin Clippy
without test seams, Rust formatting, changed-file dprint and the source sleep checker (322 delays, none unexplained).
Those results remain applicable to unchanged product/test source. The earlier store compile failure
`25a85753-907f-49aa-b6c2-19bccafddb06` is retained as a same-session namespace error corrected before the exact store
retry; it is not a passing run or a latent flake. Incomplete provider probes remain retained and are not question
coverage.

No full Rust battery, browser, desktop, installer or provisioning suite ran for this completion round. The existing
focused checks cover the application changes, and the final edits are ordinary prose. No real-provider question test was
run: that behavior is explicitly deferred. The website build supplies the previously uncovered page/link evidence.

## Review gate's outcome

All four implementation units passed the required single fresh-context gpt-6.1-sol/high gate in the prior round, after
the historical-conversion/storage and central Status documentation findings were fixed. The status unit's old overall
scope objection is superseded by the recorded smaller-scope Decision. A fresh final review of the complete amended
status unit by gpt-6.1-sol/high passed with no findings. The independent resume check also passed.
