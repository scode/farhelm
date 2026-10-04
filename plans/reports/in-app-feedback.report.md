## What this was about

You asked for a super lightweight way for users to tell you something from inside Farhelm: private, not a public issue,
no account, a sentence or two. You decided where it lives (a **?** button to the right of the settings gear, with a
small menu holding Send feedback and Documentation) and what goes out (the message, an optional contact, the version,
desktop app or web UI, and the operating system, all shown before sending). You also decided that the helm, not the
browser, makes the connection, and that it goes to the project's own endpoint at `farhelm.io/api/feedback`. That
endpoint is a Vercel function in the website project, filing each submission as an issue in a private GitHub repository
whose name and token are deployment configuration, never source. Failures keep the text and nothing is queued. Abuse
protection is size caps plus a per-IP rate limit, with a kill switch.

Four stacked PRs build that: the spec, the website endpoint, the helm route, and the UI with its docs.

## Things you should know

- **Nothing reaches you yet.** The website function has never run on Vercel; I could not deploy, and the plan forbade
  creating the repository, the token or any Vercel setting. Until you do the one-time setup, every send fails with the
  dialog's ordinary failure message ("Couldn't send feedback: the feedback service refused it (HTTP 404)." or similar)
  and the user keeps their text. The setup is in `docs/feedback-endpoint.md` (PR 2):
  - create a private GitHub repository as the inbox;
  - create a fine-grained token with Issues read and write on that repository only;
  - in the Vercel project's Production environment, set `FEEDBACK_GITHUB_REPO` (as `owner/name`) and
    `FEEDBACK_GITHUB_TOKEN`, marked sensitive;
  - add a firewall rate-limit rule on `/api/feedback`, keyed on IP (about 5 per 10 minutes is the suggested start);
  - ask for "deploy the website", then send one piece of feedback from the app and check that an issue appears.

  To turn it off, revoke the token or add a firewall deny rule for the path; both take effect at once.
- **Two deployment assumptions the repository cannot show.** Vercel serves `website/api/feedback.js` as a function only
  if the Vercel project's root directory is `website/` and the site stays a plain static Astro build with no Vercel
  adapter. Vercel's documentation says files under `api/` are deployed as functions in non-Next projects; the first
  verification send is the first real test of it. The setup document lists both checks first.
- **What a user sees.** The dialog lists everything sent besides the message and contact, and what it shows is exactly
  what the helm forwards. Send is disabled until there is a message, and cancel is disabled while a send is in flight.
  Success shows a thank-you and closes the dialog after 1.5 seconds. A failure says why in one plain sentence and keeps
  the text. Documentation opens `https://farhelm.io/docs/` in the system browser from the desktop app, or a new tab from
  the web UI.
- **Limits.** Message at most 4,000 characters (not blank), contact at most 200, and the version and operating system at
  most 64 and 128 with no control characters. Characters are counted as Unicode code points, the same way in the UI, the
  helm and the endpoint. The UI shortens the operating system and version itself, so a value the user cannot edit never
  blocks a send.
- **What reviewers changed in the design**, each now in SPEC_impl.md:
  - The endpoint accepts only JSON requests. Without that, any web page could make its visitors' browsers post feedback,
    one IP address per visitor, getting around the per-IP limit.
  - The endpoint refuses redirects from GitHub and counts only "created" as success. If you ever renamed the inbox
    repository, a followed redirect would have reported success while filing nothing.
  - Every user-written string in an issue's body sits in a code fence it cannot close, so no feedback can format the
    issue or @-mention anyone.
  - Neither the helm nor the endpoint logs the message or the contact.
  - The helm follows no redirects, and only tests can change its endpoint URL.
- **Spec changes (PR 1).** SPEC.md gains a Feedback section. The security section now says steady-state operation
  between Farhelm's own components has two network edges, and names the two connections that leave only when the user
  asks: provisioning's release downloads and feedback. Its SSH bullet now says "for reaching hosts". SPEC_impl.md gains
  a "Feedback forwarding" section with the choices above.
- **Docs.** The plan expected the user docs to go on an existing page, but no written page covers the sidebar's top bar;
  the candidates are stubs. So there is a new short page, Using Farhelm → Send feedback. The security-model stub's
  description no longer claims SSH is the only network Farhelm uses.

## Open questions and possible follow-ups

- **The docs Overview page still says "SSH as the only network"** in one of its pillars and in that image's alt text. I
  did not edit it, as the rules require. Proposed wording for both: "SSH between your machines". Do you want it?
- **Duplicate issues are possible.** If a send times out at the helm after GitHub did create the issue, a retry files a
  second one. The spec accepts this; say if you would rather not.
- **The help menu's panel stacks inside the sidebar bar's layer**, not at the level of the other menus. No overlap was
  found in testing, and fixing it means moving the panel out of the bar, so it was left.
- **Rate-limit availability.** Whether your Vercel plan includes a rate-limit rule is something only the dashboard can
  confirm; the setup document says so.

## The PRs

1. https://github.com/scode/farhelm/pull/1566/changes `docs:` specify in-app feedback and its outbound connection.
2. https://github.com/scode/farhelm/pull/1567/changes `feat:` the website endpoint that files feedback as private
   issues, its tests, and the setup document.
3. https://github.com/scode/farhelm/pull/1569/changes `feat:` the helm route that forwards feedback, and the shared
   submission type.
4. https://github.com/scode/farhelm/pull/1570/changes `feat:` the help menu, the feedback dialog, the docs page, the
   changelog fragment and the TODO removal.

PRs 2 and 3 carry `kind: none` changelog fragments, since nothing is visible until PR 4. PR descriptions are empty; the
titles and diffs carry them.

## Checks

Brackets hold the start of each run's id in the test-run recorder's retained records.

- Endpoint: its `node --test` suite, 17/17. It is new, and is now listed in the check inventory and run by CI's website
  job.
- Helm and shared type: all `farhelm-helm` and `farhelm-proto` tests, 1097/1097 [c072578c], including the route's tests
  against a stand-in server on a random port.
- UI: all `farhelm-ui` tests, 438/438 [11e3895f]; the UI's JavaScript checks, 190/190.
- Browser, Chromium and WebKit: the new feedback spec plus the header, settings and window-chrome specs that share the
  sidebar's bar, 36/36 [ed3f7715]. An earlier run caught the bar growing 1px in a narrow window [5abcac7a]; fixed. Every
  feedback request in the browser tests is intercepted before it reaches the stack's real helm.
- Clippy (all targets, and the shipped binary's configuration), the desktop UI build check, `cargo fmt`, the test-sleep
  checker, dprint, the changelog lint, and the website build with its link check: clean.

The stack is rebased onto main after your trim of the add-host dialog landed; nothing conflicted, and that change
touches only the add-host dialog. After the rebase, clippy and the feedback, menu, bar and API tests ran again, 146/146
[5e22e435]. The browser results above are reused for that reason.

Skipped: the full workspace Rust battery and the rest of the browser suite. The change touches the helm's router, a new
UI module, the bar and the shared menu-dismissal script. The crate-wide runs and the four browser specs above cover
those, and nothing touches sessions, hosts or terminals.

## Review gate

Each PR had two rounds with both reviewers you asked for, GPT-6 Astra (high) and a fresh Claude Opus (high). Every
finding was fixed except the declined points below. The fixes that mattered most:

- The size cap at the endpoint was first below what the app allowed.
- The JSON-only requirement.
- The redirect that could lose feedback.
- An interrupted upload escaped the endpoint's error handling.
- The helm's tests could have defaulted to the production endpoint.
- The UI dropped a send when closed mid-flight. It also announced success while the dialog's modal isolation hid the
  announcement from screen readers; the announcement now comes after the dialog closes.

Declined:

- Moving the help panel out of the bar's stacking layer.
- Giving the help menu its own panel classes instead of borrowing the host menu's. They are documented, and every
  existing query is scoped to a row.
