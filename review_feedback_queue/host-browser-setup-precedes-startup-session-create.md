# Host-terminal checks can require the startup session before bootstrap creates it

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A healthy host-terminal browser-validation run can fail before its startup session exists.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source path; no runtime occurrence or frequency measurement claimed.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:e2e/global-setup.ts:100,126-135;
e2e/host-terminals/migration.spec.ts:24-29,46-47.

the new expectHostTerminals function makes one GET /api/sessions and immediately requires e2e-session. At the same
immutable commit e2e/playwright.config.ts:190-192 declares the public static bundle the webServer readiness boundary,
while e2e/start-stack.sh:612-618 starts the HTTP-serving helm before its separate token exchange (623-655), local-host
wait (657-695), and startup-session POST (721-729). Global setup can authenticate and list before that POST completes.
The new premise assertion needs bounded polling for the startup-session boundary (with useful failure state), or a
readiness oracle that includes startup-session creation.

A healthy host-terminal browser-validation run can fail before its startup session exists. Exact compatible ordering at
tip: start-stack.sh publishes stack-info at578-608 and mints the stable bootstrap token at610-614; then starts the helm
at619-624. The helm claims its state, initializes auth, and starts HTTP serving
(helm/lib.rs:2248-2269,2307-2320,2353-2355). All webServer static-page probes may now succeed while the independent
stack shell is still before its token exchange (:633-661), connected-local-host wait (:673-702), create-body assembly
(:715-726) and startup-session POST (:728-736). Other slots reaching HTTP readiness does not order this shell after that
POST. Global setup can read the already-published file (:76), complete token show (:77-85: token_control.rs:213-227
takes the busy-owner read-existing branch; auth.rs:410-420 reads the existing token), exchange its independent device
secret (:87-93; auth.rs:203-225,330-346), and write welcome_seen (:95-99; preferences.rs:107-112,268-274 is a plain
sparse store write with no host/session wait). It then makes the one-shot session list (:100,126-135), which reads
existing cache/store state (sessions.rs:695-755), and asserts a row that has not been created. No startup-complete
marker or stdout wait is configured (main playwright.config.ts:163-207). Official pinned Playwright 1.64.0 (upstream
commit606ecd8b34bac100db32561b6278ffd76e5eb3cd; repo lockfile:13-20,42-49) resolves webServer plugin setup on URL
availability (webServerPlugin.ts:72-83,188-209,238-255) and then calls global setup (runner/tasks.ts:155-161,191-223).
Standalone migration path: host-terminals/playwright.config.ts:52-57 has stackSlots but no suiteTerminals host metadata,
so global setup performs auth and persists credentials (:102-112) without the host check or another session wait. The
migration file may be selected directly under config discovery (:38-39); its list happens at
migration.spec.ts:24-29,46-47 before navigation (:51). Its evidence fixtures only add passive observation and disable
hostPremise for that config (evidence.ts:46-107,128-186); built-in Playwright request/index.ts:191-204 only creates an
API request context, page/index.ts:494-505 creates a blank page. Neither waits for e2e-session. Thus every earlier
barrier is compatible with bootstrap not yet making its POST; this is a source-established possible ordering, not a
measured flake. Proposed fix: bounded startup-session readiness polling before terminal-kind assertions, or a readiness
oracle including successful session creation.

Possible coverage and caveats: FILTER.md Rare, self-correcting glitches and imprecise diagnostics, only if rarity and
applicability to safe retry of the whole validation run are established; neither follows from source, so no exclusion.
Full relevant Testing section SPEC_impl.md:6742-6808 and harness cleanup contract:2504-2515 do not accept premature
startup assertions; TODO Planned:23-25 is empty. TODO:611-625 is parent-SIGTERM cleanup, not this trigger/consequence.
BUGS.md:1-100 accepts tmux crash/idle memory and escaped SSH-transfer helpers, not this behavior. Searches of SPEC.md,
SPEC_impl.md, unlanded plans/queue, queue INDEX/matching files and TRIAGE_OUTCOMES found no matching
trigger+consequence+scope. Inspected nearby queue items browser-suite-ignores-explicit-port and
uuid-hold-intercepts-terminal-lease concern respectively port override and browser UUID interception, not session
prerequisite timing. Ledger real-agent executable, canonical trash-root, PID reuse and replacement-liveness decisions
are also distinct.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g118/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 9fcfe774c11b4bb2c57137f859f0e5545dbdc1a4.. This
was a source-only review; runtime occurrence and frequency were not measured.
