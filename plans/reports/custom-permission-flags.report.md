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
