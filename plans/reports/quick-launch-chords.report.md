# Quick-launch template composition

## What this was about

Quick launches could use only one template, and a template that only changed a model or another partial choice could not
hold a number key. That prevented combinations such as a base setup followed by “use Codex.”

The new launch sequence starts with Cmd+X in the Mac app or Ctrl+Shift+X in a browser. Number keys and the existing bolt
shortcut queue templates in order; Enter launches their combined setup, with later choices winning. A pill names the
queued templates. Ordinary number and bolt shortcuts still launch immediately, with the existing fallbacks to New when
the setup needs attention. The covered TODO entry is removed.

## Things you should know

Any template can now hold a number key. Pressing a partial template's key alone opens New with it applied and a
prominent screen-reader alert explaining composition. The explanation stays for about six seconds after application
finishes, or disappears when editing begins. The bolt's eligibility rule is unchanged.

The sequence has no timeout. Escape, a click, window blur or a dialog opening cancels it; unrelated keys cancel and
continue to their normal target. Text fields retain their ordinary prefix-key behavior, while terminal input can start a
sequence. An unbound number opens Templates only when Enter resolves the sequence. Refusals and permission questions use
the existing New flow rather than introducing another launch policy. No preference schema changed.

Linux validation cannot exercise the Mac app's Cmd+X interaction with its native Edit menu. A Mac app check remains,
including that Cmd+X starts a sequence from the terminal and still cuts inside a text field. No native menu behavior was
changed. The Launch templates, Session list and Start a session documentation pages describe the feature.

## Open questions and possible follow-ups

The Mac app check above remains platform validation for the maintainer, for example through the devbuild flow. If the
native menu intercepts Cmd+X outside text fields, its handling needs a follow-up decision; this plan does not remove
Cut. There are no other known unresolved implementation findings.

## The PRs

- [#2220](https://github.com/scode/farhelm/pull/2220/changes): prepare ordered template application through the existing
  launcher.
- [#2225](https://github.com/scode/farhelm/pull/2225/changes): compose templates with a keyboard sequence and status
  pill.
- [#2228](https://github.com/scode/farhelm/pull/2228/changes): permit partial-template number bindings and explain their
  single-key fallback.

## Checks run, reused and skipped

`npx playwright test tests/quick-launch.spec.ts tests/templates.spec.ts` passed all 146 cases, 73 each in Chromium and
WebKit, with no skips, failures or retries (recorder `38187e1e-3f90-4958-85ad-bfd12b0c2a38`). The run used one worker
and pinned tmux; it exercised owned helm/supervisor fixtures with the suite's simulated provisioning backend. Its source
capture was based on `0f420cff` plus the final third-slice working tree, subsequently committed as `4f24093e`.

The focused partial-template browser case passed in Chromium and WebKit, two cases, no skips (recorder
`a9c5cf74-8105-4f53-9267-b61aeb138f3f`). It covers binding a real catalog model through the UI, slow catalog loading,
the applied model, incidental clicks, text and button edits, expiry and no automatic create. Both banner captures were
visually inspected. Earlier sequence cases passed in both engines, six cases (`fb40ef12-73e9-4a8e-8931-250e7a6c8815`).

`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
`cargo fmt --all -- --check`, the relevant `dprint check`, changelog fragment formatting and whitespace checks passed.
The test-delay checker inspected 303 delays with no missing rationale; the agent-instruction checker reported no errors
or warnings. The docs build produced 32 pages and passed internal-link validation. The native binary build and final web
bundle build passed.

The UI library's 514 unit tests passed, no skips (`bc799848-40e3-4b10-b25b-29ea3a8c6eaa`). Its captured source was based
on `443fafe9` with the third slice in progress; later edits only corrected notice callbacks and a redundant closure,
leaving the unit-tested helpers unchanged. That evidence is reused for those helpers; browser coverage checks the final
UI behavior. The earlier two-test eligibility selection also passed (`e9f2104c-7f7d-4854-b53b-8297a2929d57`).

The asset JavaScript harness passed 286 tests, no skips (`7c20425f-d7d6-4e89-a9e3-36e3732e441b`), including 17 new
sequence cases. Reused from the sequence slice published as `443fafe9`: the JavaScript assets and tests are unchanged
since that run.

Recorded successful runs have complete reports where applicable, retained output and completed cleanup; output was
checked for runtime skip messages. Builds were reused where their inputs were unchanged. The full browser suite,
unrelated backend runtime suites, installer and release checks were skipped because the selected tests cover the changed
launch and template behavior. A Mac app runtime check was unavailable on Linux.

The keyboard-scroll audit landed while browser validation ran. The stack was carefully rebased onto main `54f43703`;
each PR's interdiff was empty. The incoming changes add keyboard entry points to text-only areas in other dialogs and
approval prompts, wrap SSH help commands, and clarify scrolling rules. They change no quick-launch listener, launcher or
template edit path; their CSS selectors do not match the sequence pill or partial-template notice. The optional scroll
label on shared text blocks does not change this feature's usage. `cargo check -p farhelm-ui` and formatting checks
passed after the rebase. Earlier runtime, Clippy and desktop evidence is reused because the rebased PR patches are
unchanged and the inspected main changes introduce no interaction needing another runtime run.

## Review gate outcome

Each PR passed a single fresh-context gpt-6.1-sol reviewer at high effort, with follow-up checks after corrections. The
first review caught an automatic launch that could use the preceding setup before the final template's choices settled.
The second corrected a test that had cancelled a sequence by clicking before it tested dialog opening. The third
corrected notice dismissal on incidental clicks and covered the existing button and keyboard choice paths. Final reviews
found no actionable issues. A separate fresh own-model scope review after the repeated notice corrections found no
substantial unnecessary complexity. Commit and PR wording passed fresh cold reads.

### Landing

Landed on its own in round 83, as #2220, #2225 and #2228, merged bottom-up and unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Only plan-queue bookkeeping reached main between this stack's base (main `54f43703`, which already included the
keyboard-scroll audit the report discusses) and its landing, so the landing rebase was clean and every PR's diff is
identical to the one the executor delivered. The base already held the related keyboard and launcher work: the WebKit
input-method guard (#2194), the existing template number shortcuts, the menu focus fix (#2176), the keyboard-reachable
scroll regions (#2190, #2222), the launcher fixes and initial prompts (#2160 to #2164, #2202, #2203) and template name
escaping (#2204). The landing monitor read the stack against them, and a fresh-context reviewer did the same
independently. Neither found an interaction:

- The new key-sequence listener is installed right after the input-method guard and reads its result: while an input
  method is composing it starts nothing and cancels an active sequence without swallowing the key. While a sequence is
  active it blocks the later number-shortcut, quick-switcher and quick-launch listeners, so a queued key cannot also
  launch on its own.
- A sequence starts only outside text fields (the terminal counts as outside), and any open dialog, click, window blur
  or unrelated key cancels it.
- Each template still goes through the same apply path as before, one at a time, so initial prompts, escaped names and
  the protection against starting a retried launch twice behave as they did. A refused template keeps its error and
  stops the rest of the sequence.
- The new script is loaded the same way as the existing number-shortcut and input-method scripts, so the desktop app and
  the browser get the same code.

The reviewer also found `refactor:` right for #2220. Its one visible difference is for two incomplete template shapes: a
template set to run a command that names an agent but leaves the command empty, and a command-only template with no
launch kind. Quick launch now opens the session launcher for them directly instead of trying to launch first and falling
back there, with the same result.

#### Fixes made during the landing

None.

#### Possible follow-ups

- The Mac app has not been checked: whether Cmd+X outside a text field reaches Farhelm before the Edit menu's Cut. The
  report already lists this as a manual Mac check.
- SPEC.md, the docs pages and the changelog entry say "Ctrl+Shift+X in browsers", but the desktop app on other systems
  uses it too; "everywhere else", as the number-shortcut wording puts it, would be exact.
- The "cannot launch on its own" banner also appears for ⚡ alone with an incomplete template, and for a one-template
  sequence, where it tells the user to use the prefix they just used. SPEC.md describes it only for a single number key.
- If a sequence that includes ⚡ fails because a number is unbound, Templates also shows the "Mark a template with ⚡"
  hint, even when one is marked.
- Templates can call a template launchable when the launch check does not, for example a template set to run a command
  that names an agent but leaves the command empty; its number key then shows the "cannot launch on its own" banner.
  Before this stack it also ended in the session launcher.
- The Templates help paragraph does not yet mention partial-template keys or sequences.

#### Checks

- Run now: none. Main gained only plan-queue bookkeeping since the base.
- Reused, from the executor's runs (the report gives each run's revision and why it still applies): the browser run of
  the quick-launch and template specs on Chromium and WebKit `38187e1e-3f90-4958-85ad-bfd12b0c2a38` (146 passed, final
  tree), the focused partial-template cases `a9c5cf74-8105-4f53-9267-b61aeb138f3f`, the UI unit run
  `bc799848-40e3-4b10-b25b-29ea3a8c6eaa` (514 passed, taken before the last small edits), the JavaScript harness
  `7c20425f-d7d6-4e89-a9e3-36e3732e441b` (286 passed, taken at the sequence slice), and the builds and static checks the
  report lists. All of these ran before the executor rebased onto the keyboard-scroll audit; after that rebase only a
  compile check and formatting ran, with each PR's diff unchanged. The landing rebase changed nothing further.
- Skipped: everything the report skipped, for the reasons it gives.

Nothing in the executor's report became untrue at landing.
