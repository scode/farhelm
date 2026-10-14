### What this was about

The session launcher offered Farhelm's built-in permission choices, but no way to supply an agent's own permission flags
while keeping Farhelm's agent launch and resume integration. Custom now offers that choice for Claude, Codex, Grok,
Muse, Cursor and OMP. The launcher previews the literal words passed to the agent. Templates, recent setups, Clone,
Replace with and Restart with retain those words for reuse or editing.

### Things you should know

Every custom launch counts as YOLO, including flags that name a guarded vendor mode. Farhelm does not interpret the
flags; the user is responsible for keeping them limited to permissions. Hosts that ask before YOLO launches still ask,
and agents cannot use custom flags to bypass that rule. Custom replaces Farhelm's permission flags in start and
supported resume commands. Quotes and backslashes group words without expanding variables or running commands. Empty
choices, incomplete quotes and Farhelm's reserved substitution words are refused.

Changing the agent clears custom and its words. A later template that replaces the permission clears the old words too.
Custom stays in recent setups but never becomes the next launch's remembered default. The CLI refuses
`--permissions custom` with template guidance; complete custom choices remain available through templates. Goose, Pi and
OpenCode do not offer custom.

The helm–supervisor protocol changes from 48 to 49. Remote hosts must be updated before the upgraded helm can use them
at all: different protocol versions refuse to connect. Older releases cannot read sessions or templates stored with
custom flags. The maintainer accepted the missing downgrade path. Existing launches without custom flags keep their
meaning and serialized fingerprints.

One coherent feature PR keeps the data, launcher and safety explanation together. The covered TODO is removed. The
documentation page is `/docs/agents/#permission-modes`; no website deployment was requested or performed.

### Open questions and possible follow-ups

No decision blocks this plan. Farhelm deliberately does not validate vendor flag meanings or infer their safety mode. No
real vendor agents were launched for validation. The browser fixture's loopback SSH host was unavailable, so these
browser results establish local helm/supervisor behavior, not remote SSH behavior.

### PRs

- [#1995 — custom permission flags](https://github.com/scode/farhelm/pull/1995/changes), draft, based on main. Head
  `89d4a3c9c3a1f00b9817e509788f1d24f44efd2c`.

### Checks run, reused and skipped

- Focused recorded `cargo nextest run` for proto, launcher, permission, template, restart and YOLO behavior:
  `f84afa45-2583-4dff-bf0f-c50d8d4d8d6a` ran 358 tests, with 357 passes and one existing template-options expectation
  missing Custom. The corrected expectation and added row, confirmation and default-memory checks passed in four-test
  run `c25a5009-b88a-40a7-a62e-a2ebe65bfdd8`. The unaffected 357 passes are reused from the development tree on base
  `d4ac0216`, recorded source fingerprint `a4af7483f6ef2ccc46b193557ccb907948050f3bc70f1bf3e37fa38abf8619d1`. Subsequent
  changes to those contracts were comments or transparent payload boxing; the boxing was checked separately. Initial
  fixture compile failure `7b986ddf-e669-406f-93a1-5daa681a69ee` and the failed expectation remain retained.
- Five frozen-fingerprint, request-digest and template-write tests passed in `3cdcb2e0-0a91-45ab-9a25-a6f0f94673cd`. One
  CLI spawn relay test passed in `179573ca-420e-4126-b3a2-cd4f9f679a65`, checking the last internal payload-boxing
  correction. Rust runs used nextest 0.9.148, four slots, zero retries and pinned tmux 3.7c.
- Recorded Playwright evidence covers 18 selected cases across Chromium and WebKit. Twelve unaffected passes from
  `5886f970-1b0c-43d9-b018-a3f07ce7ce0c` cover custom Enter, real YOLO refusal, template storage/editor switching and
  discard, Restart with, and guarded-mode regressions. Those values render identically after switching to the existing
  editable renderer. The whitespace/search cases passed on the final renderer in `cdeecd5a-f3e4-49f4-9c57-2926bedfd476`;
  all four Clone/Replace with cases passed in `c8c83ca4-ac49-4d8f-af1c-481206dd9a71`, including untouched hidden bytes
  and edited request words. Earlier records retain browser-substrate failures, an invalid fixture status word and an
  overquoted display expectation; successful reruns do not replace them.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the source-only delay checker,
  `dprint check SPEC.md SPEC_impl.md TODO.md`, changelog format and diff whitespace checks passed. The delay checker
  inspected 298 calls with zero missing rationales. Clippy is reused after the final change to one existing
  editable-renderer call, which the final web build compiled; no new lint-bearing construct was added.
- CLI/fixture and release web builds passed. The website's frozen-lockfile install and build passed, producing 32 pages;
  the page did not change afterwards. Main did not move from the plan's base, so no rebase or interaction retest was
  needed.
- Full workspace, desktop, installer, provisioning, real-agent and full-browser batteries were skipped: targeted
  compiler, guard, persistence, wire and two-engine UI evidence covers the changed behavior. No desktop shell, installer
  or host-provisioning behavior changed. Hosted CI was not triggered.

### Review gate outcome

The required fresh-context gpt-6.1-sol high source review and rechecks passed. It found stale flags text when switching
templates; the editor now resets with its saved source identity while retaining incomplete text during an ordinary edit.
Test selectors and invalid-save expectations were corrected. The final recheck found no remaining issues. A separate
required scope reassessment found no substantial expansion and identified the existing editable renderer, which was
reused. Commit and PR wording cold reads confirmed the explanation and compatibility caveats; the final title includes
the required breaking marker. All implementation remained local; only mandated reviews and cold reads were delegated.

### Landing

Landed alone in its round as #1995. While it was landing, main gained queue bookkeeping and one TODO.md entry (#1996,
the follow-up from the release-advisories decision). Neither touches launches or permissions.

#### Release-relevant

The helm–supervisor protocol moves from 48 to 49, so this release requires updating remote hosts: a host left on the old
version sits at "needs update". Neither database schema version changed, matching the precedent of #1992. A session or
template stored with custom flags cannot be read by an older release, as the plan's own report says.

#### Review

An independent fresh-context review traced the YOLO guard and confirmed that custom counts as YOLO on every path:

- a user's create, a quick launch from the marked template, and templates;
- Restart with;
- a plain Replace, which asks on hosts that ask first;
- agent-requested creates, clones and inherited spawns, which are refused on hosts that ask first;
- plain restart, which by design is not re-checked.

Skipping session confirmations cannot skip the YOLO question. Custom words are placed as whole arguments with no shell,
and the reserved placeholders are refused. Custom never becomes the remembered permission on either side.

#### Fix made while landing

The review found one blocker, fixed before merging. An agent's request to write or edit a launch template showed only
"permissions: custom" on its approval card, never the flags themselves. An agent could store arbitrary custom flags in a
template this way, or edit a custom template and swap only its flags, leaving the card looking the same. Such flags can
make the agent run arbitrary commands, for example through hooks given in a settings flag. Launching that template, or
quick-launching it if it carries the mark, would then start it with no further prompt on a host that starts YOLO
sessions without asking. The approval covers what the card shows, so the card now shows the exact words, as the launch
card already did, and a new unit test pins it. The YOLO question for a custom launch also no longer says "your custom
permission flags", since an agent may have supplied them; the browser test that pinned the old wording was updated.

#### Review notes left as they are

- The YOLO question does not list the custom flags it is asking about. Showing them would make that confirmation more
  meaningful.
- Length and count limits on custom words are enforced only by the supervisor. Very long flag lists therefore fail after
  the YOLO question, with a message about resume command elements rather than permission flags. A helm-side limit would
  give a clearer refusal.
- Only NUL is refused among control characters. That is harmless, since no shell runs the words and the display escapes
  them.
- Words like `--` or a vendor subcommand are not refused. That escalates nothing, since a custom launch is already YOLO
  and the spec leaves such words to the user.
- Launching with custom clears the remembered permission rather than leaving the previous one. That is the safe
  direction.
- Agents see custom words in their template listing, while command text is withheld there; the spec does not say which
  is intended.
- On the agents page, the custom paragraph comes before the guarded and YOLO modes are explained.
- The changelog fragment's wording will be rewritten at release curation.

#### Checks

- Run now, on the final tree:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog format lint and the sleep checker (298 delays, none
    missing a reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
    `cargo check -p farhelm-ui --features desktop`;
  - the supervisor, helm, UI, protocol and CLI unit and binary tests, plus the structured launch, agent relay, wrapper
    launch, restart-with-resume, create idempotency, GitHub checkout and session lifecycle e2e modules, through the
    recorder with pinned tmux, four slots and no retries: 3133 passed, run `e19fbb4b-74af-4d13-acd8-91946caa3abd`. Its
    224 skips are tests outside the selection;
  - helm, protocol and CLI doctests, run `07b31069-9f75-4af3-9e84-fb45011ca40f`;
  - the asset JavaScript harness (241 passed) and the website build;
  - on Chromium and WebKit, the custom permissions, approval layout, guarded permissions, YOLO guard, Restart with,
    clone, templates, remembered permissions, quick launch and launcher Enter specs: 258 passed, none failed, run
    `dcf806fa-80c5-4aad-a34d-998616292b98`.
- Stopped on purpose: an earlier start of the same run, while the blocker fix was edited in.
- Not run: the full workspace battery, the full browser battery, desktop runtime, installer and release gates. This
  change touches none of those paths.
- No real vendor agent was launched with custom flags, here or by the executor.
