# Codex conversation continuity after Replace

The report remains unresolved. Source inspection has not established a Farhelm defect, and a different Farhelm row ID
alone would not prove that Codex started a fresh conversation. This investigation tracks launch arguments and captured
vendor identities separately.

Observed on 2026-09-08. Source inspection started at `722a690f4ee06dbf7350519811ddec0bcc3e5413`; runtime probes used a
development build reporting Farhelm `0.5.0-rc.3`, with the concurrent host-selector simplification applied in progress.
The tested CLI SHA-256 was `233dd5cf70cfade43397e5b519a6f179665bf32ad38db83ad81421cf66e62854`. The operator retained the
exact CLI/web artifacts, private diagnostic fixtures, and recorder evidence under the run IDs below. These exploratory
fixtures are not maintained regression tests.

## Report and scope

The reported client was the macOS desktop app, with a local helm and an agent on a remote Ubuntu machine. After Replace,
the replacement could summarize earlier work and appeared to retain the previous conversation. No incident launch
arguments, profile contents, conversation IDs, exact Codex version, or exact Farhelm build were supplied. Whether
Restart preceded Replace is unknown.

The investigation uses synthetic conversations and an empty working directory in an owned Ubuntu 26.04 container. The
browser stack has separate helm and supervisor processes, with a second supervisor reached through loopback SSH. That
exercises remote routing, but it is not the original two-machine topology or native macOS desktop client.

Restart and Replace were sent directly to the HTTP API; the browser supplied prompts after a reload and explicit
attachment, so these runs do not exercise the row actions or the ordinary terminal handoff.

## Source trace

In [`sessions.rs`](../crates/farhelm-helm/src/sessions.rs), `do_replace_session` reads the source from its host,
resolves the launch mode, creates the replacement, and then deletes the source. A failed create leaves the source
intact; uncertain deletion is reported explicitly. Replace does not invoke the supervisor's Restart operation.

`mode_from_source` selects the original invocation for a raw session. For a profile-backed session, it uses the current
profile by identity while that profile exists; a deleted profile falls back to the stored invocation. That is the
specified profile contract, rather than a new interpretation of freshness.

The supervisor stores the invocation separately from its integration snapshot and resume template. In
[`service/core.rs`](../crates/farhelm-supervisor/src/service/core.rs), `relaunch_argv` selects the filled resume argv
for Restart with Resume, while a fresh launch parses the stored invocation. Restart does not replace that stored
invocation with the transient resume command. A replacement starts with no captured conversation; it does not inherit
the source's captured identity. Its own capture proceeds through a launch report or fallback record correlation.

Consequently, the hypothesis that Restart mechanically overwrites the original invocation with `codex resume <id>` and
Replace then reuses it is contradicted by this source trace. That does not prove which command ran during the reported
incident or what Codex did with it.

## Runtime observations

The controlled-agent comparison passed in run `dcffa269-6675-42b7-93e1-ac64661031ed`. The fixture allocated an identity
on a fresh launch, reused only an explicit `resume <id>`, and executed Farhelm's actual injected SessionStart hook. The
sequence was fresh launch, Restart with Resume using the same identity, then Replace with a new identity and no resume
arguments. Host, directory, title, and the ordinary `--yolo` invocation were preserved. Identity was read from the owned
remote supervisor's database, and host ownership from the session listing. This request-only comparison does not
exercise a browser or real Codex semantics.

Two earlier attempts failed in the private diagnostic: `5d0517d5-052b-431d-a445-0c4700b6f445` omitted Replace's required
JSON content type; `84bb8e57-f9da-4610-93df-1d7ed5bdd9f9` expected listing-only host metadata in a lifecycle reply. Both
failed attempts were retained. Correcting those fixture errors did not change Farhelm.

With real Codex CLI 0.153.4 under browser WebKit, ordinary Replace passed in `898c03ef-bc66-4952-89dc-ef1211842ec8`: two
launches without resume arguments reported different captured IDs. The stronger after-Restart comparison passed in
`75d9f5e0-f0a1-4dae-80db-64efddd56f1b`. The source accepted a synthetic `amber` prompt; Restart launched with that
source ID and accepted a `cedar` prompt into the same vendor rollout; Replace launched without resume arguments and
accepted `birch` into a different rollout. The vendor's own `session_meta` and exact synthetic user-message records
corroborated the captured IDs. Original `--yolo` invocation arguments survived both operations.

Run `d1795826-4e5e-4b98-930f-39c6335c5670` had already shown the same launch/identity transition, but replaced the
resumed process before proving it accepted another prompt. It supports the narrower startup-adjacent observation.
Attempt `af432b43-0547-420d-acd0-7a9a8b9279db` failed before the first prompt: the diagnostic mistook old loading
banners in terminal scrollback for the current composer state. The trace showed the latest model/composer already ready.
The later fixture checked the latest model row; no product behavior changed.

Chromium run `550c3463-8e9e-4c84-a511-53ab94edae70` passed ordinary Replace and Restart with Resume followed by Replace.
Both comparisons produced distinct replacement vendor IDs and preserved the original invocation. The after-Restart case
corroborated the synthetic source, resumed, and replacement messages against vendor records, using the same stronger
oracle as the later WebKit run. Launch shapes were fresh/fresh and fresh/resume/fresh, respectively. This run used
Chromium 151.0.7922.34; the WebKit runs used WebKit 26.5, both through Playwright 1.62.0 with pinned tmux 3.7c.

## Remaining uncertainty

A raw invocation, current profile, or wrapper can itself request resumption; Replace preserves that launch choice. No
incident configuration establishes that explanation here. Changing arbitrary invocation parsing to strip resume
arguments would therefore be speculative and could damage explicitly configured commands.

Repository instructions, files, or other supplied context can also let a fresh conversation describe previous work. The
model's ability to summarize is useful incident evidence but does not distinguish shared context from reuse of a vendor
conversation. Conversely, even a successful fresh-identity comparison in this environment cannot rule out a
version-specific or desktop-specific failure in the reported environment.

For a recurrence, retain the source and replacement vendor conversation IDs, the sanitized actual launch arguments,
whether Restart occurred first, and the profile or wrapper that resolved at Replace time. If Replace emits an unexpected
resume command, trace the invocation selection. If it emits a fresh command but the vendor reports the old identity,
compare a direct launch with the same isolated configuration before changing Farhelm. The TODO remains open until
evidence establishes a cause and, where applicable, a validated fix.
