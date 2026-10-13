# Open remote-session links in your browser

## What this was about

An agent on a remote host could try to open a link and fail because that host had no browser. The requested behavior was
to open it on your machine instead, asking first, with separate per-host choices for public and local-network links.

The draft stack supplies a remote-session browser fallback for both agents and terminal tabs. Link requests show the
session, host and full URL, and accept only HTTP or HTTPS. The Mac app opens an approved link in your default browser;
the web UI opens it during your click. Both new permissions start off, remain independent of command permissions, and
return to off when you adopt a replaced host. Always allow grants only the link's category to the installation that
asked.

## Things you should know

With the matching permission on, the Mac app opens immediately; the web UI still needs one click on Open, and Dismiss
refuses. Allowing local-network links lets that remote host make your browser load addresses on your own machine or
network without another approval. Classification reads the URL's host without resolving DNS, so a public-looking name
can still resolve to your private network.

A running agent gets the fallback at its next launch, and a new terminal tab gets it immediately. Existing terminal tabs
keep their environments. Your own BROWSER setting wins, including an empty value. This-machine sessions keep their
current browser behavior. Farhelm omits the fallback for install paths common browser readers cannot represent safely.

Vendor inspection found a limitation the outline's generic browser-reader assumptions did not cover: Claude Code 2.1.296
on Linux expects BROWSER to name one executable, so its own opener cannot use this command template. An agent can invoke
`farhelm agent open-url -- <url>` directly. Codex 0.162.1 uses a browser library that starts the command in the
background, so it can report success before you answer and cannot observe a later refusal. Python's template reader and
xdg-open wait for the command's result. No external browser was opened during validation.

A login redirect to localhost on the remote host still cannot finish in your local browser. Forwarding the link does not
connect the remote callback server. A web click can open the link before the helm later refuses an expired or
already-answered card, as accepted in the plan; the desktop opens only after admission. Success means the opener
accepted the request, not that the destination loaded. Web pop-up blocking cannot be detected reliably.

The connection first accepts the host and waits for forwarding to be applied before offering launches and tabs. Rejected
or duplicate hosts cannot change it. A reconnect to this machine explicitly clears remote forwarding. This policy needs
no stored session migration; the two default-off permissions use the helm's schema-45 migration.

## Open questions and possible follow-ups

No decision blocks this stack. Claude Code's native opener may change independently; the documented version's limitation
can be revisited then. Remote localhost login callbacks need a separate connection or the agent's headless/device-login
flow. Ordinary documentation screenshots can be refreshed separately; published release screenshots were untouched.

## The PRs

- [#1944 — separate host browser grants](https://github.com/scode/farhelm/pull/1944/changes): default-off public and
  local-network permissions with adoption resets and identity-bound grants.
- [#1948 — open agent links with host approval](https://github.com/scode/farhelm/pull/1948/changes): manual command,
  approval cards, settings controls and web/native opening.
- [#1950 — forward remote session browser links](https://github.com/scode/farhelm/pull/1950/changes): remote agent/tab
  forwarding, user documentation and removal of the covered TODO entry.

The linear draft stack's pushed heads are `50a2758a1114`, `5c4b182be086` and `4a7c08f32d9e`, respectively.

## Checks run, reused and skipped

Run for the forwarding unit:

- Recorded run `1da0d91f-e1d9-4520-bf77-563e1b5736cf`: all 13 selected protocol, placement, deadline/refusal, identity,
  restricted-authority, unsafe-path and environment cases passed. Run `ad917021-8c22-464b-bff6-0c1dcc2417c6` passed all
  four selected applied-acknowledgement, real browser CLI, agent-relaunch and new-tab login-environment cases, with no
  runtime skips. The CLI case includes Allow, Deny, Dismiss, no-GUI refusal and a rename while a link waits.
- Both Clippy configurations passed: all targets and the shipped CLI without test seams. Formatting, changed Markdown,
  changelog lint and the isolated delay checker passed; 296 delays had zero missing reasons. The website built 32 pages
  and validated every internal link. Its first attempt hit a pre-existing unwritable generated release-notes file;
  regeneration resolved that local artifact issue without changing release-note source.
- Pinned Codex source and its browser library, the installed Claude Code bundle, Python's browser controller and
  xdg-open were inspected to establish template parsing and wait/background behavior.

Reused for the combined stack:

- Storage run `7ff1af1f-3c32-4d79-b416-e9afe1cdff16`: six permission/migration/parity cases.
- Manual-link run `fbdc7c88-2856-4319-8014-e8583012e30d`: 21 passing URL, category, wire and route cases; its two setup
  failures were corrected. Five edge/host-view cases passed in `f7bd19f7-2120-4e59-8617-ceab76dddd28`; the final CLI
  assertion passed alone in `f0c95e92-7476-4753-a83f-94e5a245847f`.
- Browser run `0198c043-3343-467b-a500-1ba206fa907c`: 22 cases passed, 11 each on Chromium and WebKit, covering full
  approval layout, user activation, refusal/dismissal, guards, invalid cards and independently persisted settings.
- CLI/fixture build, pinned web release build, desktop renderer and desktop binary compile checks passed before
  forwarding. Their UI/native-opener behavior is unchanged; the new placement code is compiled by the final Clippy and
  targeted Rust selections. No browser or native-opening code changed after those passes.

The earlier careful rebase examined Codex process-attribution and supervisor-schema work, feedback layout and shared
bookkeeping. They did not change the link relay, approvals or launch environment; schema 31 is the supervisor's,
separate from the helm's 45. The final comparison also read the app-settings landing: its optional header tile leaves
host settings unchanged, and its scoped layout CSS affects app settings alone. All three units rebased cleanly onto main
`892cd2f8`. A focused UI all-targets Clippy check covers the shared component API; its landing's recorded both-engine
layout coverage is additional evidence. The final diagnostic/comment/heading corrections change no tested behavior.
These results cover the same code on the rebased stack; no new runtime interaction remained.

Failed development records are retained privately: `c7fcc240-7ef1-477e-a16e-3069ead8c133` and
`ef94ef2e-bcb5-49cc-bf3c-3893bbb57565` were compile failures from incomplete fixtures;
`fbdc7c88-2856-4319-8014-e8583012e30d` and `f7bd19f7-2120-4e59-8617-ceab76dddd28` exposed test setup/assertion errors
fixed in this session. `369817f3-a36d-4cab-a432-a9440980d997` also stopped at compilation on four incomplete
placement-log fixtures. No runtime cases ran in those compile failures. Later passes are separate observations, and
these were not logged as latent flakes.

Skipped: full Rust/browser batteries, desktop runtime, installer, provisioning and release gates. The identified risks
are URL approval, placement, launch environment and UI opening, covered by the selected checks; no broader interaction
requires those suites. CI is on demand and was not dispatched; no website deployment or release publication was
requested.

## Review gate's outcome

Each code unit had the required fresh gpt-6.1-sol review at high effort, with the full plan and verbatim test-authoring
contract. Storage review corrected historical schema fixture rewinds; the manual unit corrected a denial-wording test
assertion. The forwarding review found one misleading timeout diagnostic, corrected and independently verified. No
review finding remains open. Each commit and PR message had a fresh cold read; the final forwarding wording explicitly
includes Claude Code's limitation after its first reader identified that omission. All touched files received a separate
documentation pass. Implementation and validation were local; only the required reviews were delegated. All PRs remain
drafts; none was marked ready or merged.
