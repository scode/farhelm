# In-app feedback: a private "Send feedback" box that reaches the maintainer

Written against main at a4e3e169 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan amends them (PR 1 below) where it adds behavior they do not yet describe.

## The goal

Give Farhelm users a super lightweight way to tell the maintainer something: open a small dialog, type a sentence, press
send. It is private (not a GitHub issue anyone can read), it needs no account, and it goes only to the maintainer.
Opening a public issue stays possible, but this is the low-friction path for a one-line remark.

The pieces:

- **The help menu.** A new **?** icon button in the sidebar's top bar, immediately to the right of the settings gear,
  opens a small popup menu with two items: **Send feedback** and **Documentation**. Documentation opens
  `https://farhelm.io/docs/` (the docs site) in the user's browser: a new tab in the web UI, the system browser from the
  desktop app. Both the desktop app and the web UI get the menu. (The gear itself opens a modal dialog, not a menu; the
  popup-menu machinery to reuse is `crates/farhelm-ui/src/menu_panel.rs`, today used by the session and host row menus.)
- **The feedback dialog.** A text box for the message (required), an optional field for how to reach the user, and a
  plain display of everything that will be sent besides those: the Farhelm version, whether this is the desktop app or
  the web UI, and the operating system. Send and Cancel. On success the dialog says thanks and closes. On failure
  (offline, the endpoint unreachable, not deployed yet, refusing, rate-limited) it says sending failed, in plain words,
  and keeps the typed text so the user can retry or copy it. Nothing is queued or retried later.
- **The helm forwards it.** The UI composes the whole submission (message, contact, version, desktop/web, OS) and posts
  it to its own helm; the helm checks sizes and forwards it unchanged to the feedback endpoint. What the dialog shows is
  therefore exactly what is sent. Default endpoint: `https://farhelm.io/api/feedback`.
- **The endpoint.** A small Vercel serverless function in the existing `website/` project (which already deploys to
  `farhelm.io`), at `/api/feedback`. It validates the submission, enforces size caps, and creates an issue in a private
  GitHub repository using a token held only on the server. Which repository, and the token, come from the Vercel
  project's environment configuration; neither appears anywhere in the source. When configuration is missing it fails,
  which the app shows as an ordinary send failure.

This is the first time Farhelm sends user-written content to a service the project runs. In steady state the helm talks
only to the user's browser and the user's own supervisors (SPEC.md "Security": "Steady-state operation has exactly two
network edges"); the one other outbound contact today is provisioning downloading release payloads from GitHub
(`--release-base-url` overrides the source). Feedback must therefore leave the machine only when the user presses Send,
the dialog must show exactly what goes out, and the spec and the user docs must say so.

Acceptance criteria:

- The **?** button sits to the right of the gear in the sidebar top bar, in the web UI and the desktop app, with an
  accessible name and hover text consistent with its neighbours. Its menu holds exactly Send feedback and Documentation,
  and follows the existing popup-menu conventions (keyboard, focus return, dismissal) of the other sidebar menus.
- Documentation opens `https://farhelm.io/docs/` through the UI's existing external-link path, in both surfaces.
- The feedback dialog behaves as described above: message required, contact optional, the automatically attached fields
  displayed with their actual values before sending, success closes it, failure keeps the text and shows a plain failure
  message.
- A new helm route accepts the UI's feedback request behind the same device authentication as every other protected UI
  route, validates sizes, and forwards the submission unchanged as JSON to the configured endpoint with a bounded
  timeout. The endpoint URL defaults to `https://farhelm.io/api/feedback` and is injectable, so Rust tests point it at a
  stand-in server they start themselves. No test sets or changes the test process's own environment variables (user
  rule; use dependency injection).
- Agents cannot send feedback: the route is a UI route only. Agents reach the helm solely through the supervisor relay's
  fixed verbs (`crates/farhelm-helm/src/agent_requests.rs`), so do not add a relay verb or a `farhelm` CLI verb for it.
- The function in `website/` creates one issue per accepted submission in the configured private repository: a short
  title derived from the message, a body with the message, the optional contact, and the attached metadata. It reads the
  repository (`owner/name`) and the token from environment configuration, refuses with a failure status when either is
  missing, rejects malformed or oversized submissions, and never echoes the token or repository name to the client. Its
  logic is unit-tested with injected configuration and an injected GitHub client, not by mutating `process.env` in the
  test process.
- `cd website && bun install --frozen-lockfile && bun run build` still succeeds; the static site is unaffected.
- SPEC.md describes the feature and amends the network-edges statement; SPEC_impl.md records the implementation choices
  (helm forwarding, the endpoint, its configuration, size caps).
- A maintainer setup document lists what the maintainer must do once to make it live (below, "What the maintainer
  deploys").
- The user docs on the website say how to send feedback, that it is private and goes only to the maintainer, and exactly
  what is sent; the security-model page (`website/src/content/docs/docs/how-it-works/security-model.md`), which says SSH
  is the only network Farhelm uses, is corrected to mention the feedback connection.
- The Near term TODO.md entry "In-app feedback" is removed in the last code PR, and a changelog fragment exists.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term), verbatim: "A super simple way to give feedback on Farhelm from inside the app.
  Where the feedback goes and what the UI looks like are details TBD."
- M2. In the planning conversation, the maintainer's words: "So obviously we can just tell people to open a GitHub
  issue. The thing that I really want is to have that super, super, super lightweight feedback mechanism that goes just
  to me. It's not public, people don't have to worry about it. They can just write a little one sentence or something
  like that. In the future we might add LLMs poking around and digging around and filling it in with additional
  information and stuff like that." LLM enrichment is future work and out of scope here; the JSON payload should simply
  leave room for more fields later.
- M3. Destination: the project's own endpoint under `farhelm.io`, not a third-party form service, Sentry, or ntfy. The
  URL is one the maintainer controls forever, so the backend can change without a release.
- M4. Payload: the user's text, plus the Farhelm version and desktop/web and OS attached automatically, plus an optional
  contact field; all shown before sending.
- M5. The helm forwards; the browser and the desktop webview never post to the internet themselves.
- M6. The endpoint is a Vercel function in the existing website project at `farhelm.io/api/feedback`, and the inbox is a
  private GitHub repository: each submission becomes an issue there, created with a server-held token scoped to that
  repository. No ntfy ping; GitHub's own notifications are enough.
- M7. The maintainer's words: "dont hard code private repo name in source code we need that ot be some kind of config".
  The repository name and the token are deployment configuration (Vercel environment settings), never source.
- M8. Entry point, the maintainer's words: "sidebar nexgt to gear itom (to the right of it), that is some general
  help/about dialog (maybe ? icon), and popup menu then has "Send feedback"." The menu also has a Documentation link.
  The version already shown in the bar stays where it is; no About dialog.
- M9. On failure: show an error and keep the text; no queueing in the helm.
- M10. Abuse protection for v1: a size cap and a per-IP rate limit, accepting that a determined spammer could still fill
  the inbox, with a kill switch to turn the endpoint off.
- M11. Review gate: two fresh-context reviewers per PR, a Claude Opus 5.5 agent at high effort and a gpt-6-astra agent
  at high effort, both with the general review charter (below). No review swarm.
- M12. No-workhorse mode (below).

Binding repository rules:

- Root `AGENTS.md`: Conventional Commits; a `feat` PR carries a changelog fragment; the TODO entry is removed in the PR
  that addresses it; "Finishing work" for validation; `.agents/test-authoring.md` for test changes;
  `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when Rust or browser tests change; browser
  specs run on Chromium and WebKit through the recorder; SPEC.md and SPEC_impl.md are authoritative and any change of
  behavior they specify is surfaced, which PR 1 does on purpose.
- Root `AGENTS.md` "Vercel deployments": the website deploys only when the maintainer asks. Do not run
  `website/scripts/vercel-deploy.sh`, `vercel` or any other deployment, and do not create GitHub repositories, tokens,
  or Vercel settings. Those are the maintainer's (below).
- Root `AGENTS.md` "Sharing the machine with other agents": stand-in servers in tests bind port 0; no fixed ports or
  paths.
- `website/AGENTS.md` and `website/EDITORIAL_RULES.md` govern any change under `website/`; read both first. The docs
  Overview page (`website/src/content/docs/docs/index.mdx`) is not edited.
- Root `AGENTS.md` "Harness-specific code" does not apply (nothing here is per harness).
- The project may become public: no personal names, hosts, or the private repository's name anywhere in the repository,
  including docs and tests (use placeholders like `owner/feedback-inbox`).

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION):

- P1. Agents are excluded by construction (acceptance criteria). Name that in SPEC.md so a later relay verb does not add
  it by accident: an agent sending feedback would be content leaving the user's machine without the user pressing Send.
- P2. Size caps: message at most 4,000 characters, contact at most 200; whitespace-only messages are refused. The
  request type and the caps live in `crates/farhelm-proto` (both the helm and the UI depend on it) so the UI and the
  helm enforce the same numbers; the function enforces them again and also caps the raw request body (for example 16
  KiB), since it is the public boundary. Exact numbers may change if the code suggests better ones; log it.
- P3. The per-IP rate limit is a Vercel firewall (WAF) rate-limit rule on `/api/feedback`, configured by the maintainer
  in the Vercel dashboard, so blocked traffic never reaches the function. Vercel's documentation and changelog say the
  Hobby plan includes one rate-limit rule per project (fixed window, up to 10 minutes). The function implements no
  limiter of its own.
- P4. The kill switch needs no code: revoke the GitHub token (immediate) or add a firewall deny rule for the path. Do
  not build a disable variable; Vercel applies environment variable changes only to new deployments, so it would not be
  immediate anyway. The setup document says how to turn it off and back on.
- P5. Environment variable names in the function: proposed `FEEDBACK_GITHUB_REPO` (`owner/name`) and
  `FEEDBACK_GITHUB_TOKEN` (a fine-grained personal access token with Issues write on that one repository only). The
  executor may rename them; the setup document is the source of truth.
- P6. The function is plain JavaScript (not TypeScript, so `node --test` runs the shipped file directly) with no runtime
  dependencies: `fetch` against the GitHub REST API suffices. Split it into a pure handler (request + config + GitHub
  client in, response out) and a thin Vercel entry that reads `process.env` and passes the real client. Proposed file:
  `website/api/feedback.js`, because `website/vercel.json` marks `website/` as the Vercel project root; the repository
  cannot confirm the project's root-directory setting, so the setup document states that assumption. Verify against
  Vercel's documentation that a root `api/` directory serves a function in an Astro static project; if it does not, use
  what Vercel documents for Astro only if the rest of the site stays static and the build is unchanged, and log the
  DECISION. Test the pure handler with `node --test` from a test directory under `website/`, the way
  `crates/farhelm-ui/js-tests` is run, and add that suite to the check inventory in root `AGENTS.md` (Finishing work)
  and to the existing node job in `.github/workflows/ci.yml` (the inventory must match the workflow). It must stay out
  of the Astro build. You cannot deploy, so the report must say plainly that the function has not run on Vercel yet.
- P7. Issue shape: title `Feedback: <first line of the message, trimmed to about 60 characters>`; body sections for the
  message (quoted or fenced so Markdown and @-mentions in it cannot spoof the metadata or ping anyone), contact (or
  "none"), and a small metadata table (Farhelm version, surface desktop/web, OS). No labels: the private repository is a
  dedicated inbox.
- P8. Helm side: a new route, for example `POST /api/feedback`, alongside the other protected UI routes in
  `crates/farhelm-helm/src/lib.rs`, in its own small module. The helm already depends on `reqwest` with rustls. Use a
  timeout of about 15 seconds. Failures follow the helm's existing route-error convention: one plain-text message the UI
  shows ("Couldn't send feedback: ..."), no error vocabulary and no special case per status. Log failures without the
  message text. The endpoint URL is a parameter of the helm's state construction, defaulting to the production URL; Rust
  tests construct the helm with a stand-in URL. No user-facing CLI flag, config key, or environment setting for it.
- P9. The UI composes every field: the version it already displays (`displayed_version` in
  `crates/farhelm-ui/src/app_bar.rs`, which is the helm's version when they differ), desktop or web (it knows which
  surface it is), and the operating system of the machine the user is on, from what the browser or webview reports. No
  new helm route to read the helm's own OS.
- P10. The **?** button follows the gear's markup and styling in `crates/farhelm-ui/src/app_bar.rs`; its menu uses
  `crates/farhelm-ui/src/menu_panel.rs`. The pending plan `hover-help.md` will later give every control a themed tooltip
  and a browser test that fails on any control without one; give the new button and menu items the same kind of hover
  and accessible text the gear has today, so either landing order works.
- P11. Browser coverage: one Playwright spec (Chromium and WebKit) that opens the menu and the dialog, and covers the
  success path and a failure path (text kept) by intercepting the helm's feedback route with `page.route`, as other
  specs already do for helm API calls. No stand-in endpoint and no test-only setting in the shipped helm; the Rust tests
  cover forwarding. Documentation link: assert the target and the opener call, not a real navigation.
- P12. Docs: a short section on an existing user-facing page that fits (follow `website/EDITORIAL_RULES.md` and
  `website/AGENTS.md`), saying where the menu is, that feedback is private and goes only to the maintainer, exactly what
  is attached, and that nothing is sent until Send is pressed; plus the security-model page correction. No screenshots
  (the docs-shots flow is the maintainer's). Do not edit the docs Overview page or the intro SVG words: its pillar says
  "SSH as the only network"; if you judge it now misleading, say so in the report as a proposed edit for the maintainer.
- P13. The maintainer setup document: `docs/feedback-endpoint.md` (or a section in `website/AGENTS.md` if that reads
  better; log it). It lists, as steps: create the private repository; create the token with Issues write on it only; set
  the environment variables in the Vercel project's production environment; add the WAF rate-limit rule on
  `/api/feedback` (suggested starting point: about 5 requests per 10 minutes per IP); deploy with the existing
  `website/scripts/vercel-deploy.sh main`; verify with one test submission from the app; how to turn it off and on. No
  private names in it.
- P14. Commit types: PR 1 `docs`; the endpoint PR and the helm PR `feat` with a fragment of kind `none` (nothing
  user-visible until the app uses them); the UI PR `feat` with a fragment of kind `added` describing the menu and the
  privacy of feedback. Adjust if the slicing changes; every `feat` PR has a fragment.

## Implementation outline

Four PRs in this order; merge or split further only if the code argues for it, and never in a way that adds code one PR
and removes it in a later one:

1. **Spec** (`docs`): SPEC.md gets a short "Feedback" section (the help menu, the dialog, what is sent, privacy, only on
   Send, UI-only so agents cannot send) and an amendment to "Security"'s network-edges statement adding the feedback
   connection: the helm to the project's feedback endpoint, only when the user sends feedback. SPEC_impl.md records P2,
   P3, P5, P6, P8 at the level it records similar choices.
2. **Endpoint** (`feat`): the function under `website/`, its pure handler and `node --test` tests, the inventory and CI
   entry for that suite, and the maintainer setup document (P13). The website build still passes.
3. **Helm route** (`feat`): the shared request type and caps in `farhelm-proto`, the new route and module, size
   validation, forwarding with an injectable endpoint URL and timeout, Rust tests against a stand-in server on port 0
   (success, refusal, unreachable, timeout, oversized input refused before any outbound call).
4. **UI** (`feat`): the **?** button and menu, the Documentation link, the feedback dialog and its API call, the browser
   spec (P11), the user docs section and the security-model correction (P12), the changelog fragment, and removal of the
   TODO.md entry.

Nearest existing patterns: the settings gear in `crates/farhelm-ui/src/app_bar.rs` (its dialog is in
`crates/farhelm-ui/src/settings.rs`); `crates/farhelm-ui/src/menu_panel.rs` for popup menus; protected UI routes in
`crates/farhelm-helm/src/lib.rs` (for example `/api/preferences`); the shared link opener registered in
`crates/farhelm-ui/src/lib.rs` (`terminal-links.js`); the JS unit harness in `crates/farhelm-ui/js-tests`.

The cost drivers are the endpoint (new code in a new place, untestable against real Vercel here) and the dialog. There
is no new persistent state anywhere in Farhelm: the helm stores nothing about feedback.

### What the maintainer deploys

After the PRs land, the maintainer follows the setup document. Until then, Send fails with the ordinary failure message,
which is the agreed behavior (M9). Say this in the report, and list the setup steps there too.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-in-app-feedback-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/in-app-feedback/<nn>-<short-name>`.
- A linear stack of bite-sized PRs per the outline. Within this run, a PR that needs correcting is restructured rather
  than corrected on top, including PRs an earlier run built.
- Conventional Commits; every `feat` PR carries its changelog fragment under `releasing/changelog.d/` in the same
  commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- Remove the Near term TODO.md entry "In-app feedback" in the last code PR.
- Never edit `plans/`.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
`cargo clippy -p farhelm --bins -- -D warnings`; `cargo check -p farhelm-ui --features desktop`; a nextest selection
covering the new helm module and any touched UI modules; the new `node --test` suite under `website/`; the UI JS harness
if an asset JS file changes; `cd website && bun install --frozen-lockfile && bun run build` for the endpoint and docs
PRs; `dprint check` on changed files; `python -B scripts/check-test-sleeps.py` when Rust or browser tests change; and
the new browser spec on Chromium and WebKit through the recorder (browser evidence is required for the UI PR, since the
dialog and the menu are the feature). Say in the report what ran, what was reused, and what was skipped and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes to TWO fresh-context
reviewers, as the user demands (M11): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at high effort
(shelled out to the other harness if the executing one cannot reach that model natively). No review swarm. Both get the
same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. When the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim. For the
endpoint PR, also ask the reviewers to look specifically at what an anonymous caller can make the function do (input
handling, Markdown or mention injection into the issue, token or repository leakage in responses or logs). Address what
both reviewers find before moving on, and log the DECISION where you decline a finding. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: persistent feedback
storage or a retry queue in the helm, a new dependency for the endpoint, an in-function rate limiter, a disable
variable, a test-only endpoint setting in the shipped helm, a helm route to report its own OS, a CLI or relay verb for
feedback, attaching logs or session content, an About dialog), and whenever the same component has needed repeated
corrective review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the request, the
decisions above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which
simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the function lives and how Vercel picks it up (P6), the environment
variable names (P5), size caps (P2), how the helm's endpoint URL is injected (P8), how the UI determines the OS (P9),
the issue shape (P7), where the docs and setup document went (P12, P13), and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. Agreed fallbacks: if Vercel's documentation shows the root `api/` directory cannot serve a
function in this Astro project, use the documented Astro route per P6 if it keeps the site static; if neither works
without changing how the rest of the site builds or deploys, block per `plans/AGENTS.md` (Executing one plan, step 10)
with the options, after pushing the spec and helm PRs that do not depend on it (the helm PR depends only on the
endpoint's JSON contract, which PR 1 or the helm PR can state). Never deploy anything, and never create a repository,
token or Vercel setting to test against.

## Done criterion

The plan is complete when its draft PRs exist, satisfy the acceptance criteria, have each passed the review gate, and
the last one has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed
this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
