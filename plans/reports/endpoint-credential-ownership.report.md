# What this was about

A Tensorlake recipe could choose a new API endpoint while keeping credentials already saved in its image. Codex could
send the image's old key or extra headers to that endpoint. Claude Code could keep old gateway headers, and an image's
API key or credential helper could override a recipe's subscription login.

The maintainer chose one credential owner per agent: Farhelm supplies them for API endpoint or Subscription; None leaves
them to the image and host. Mixing these sources gives undefined results. The implementation nevertheless removes the
specified conflicts as a small best-effort safeguard. No new setting was added.

# Things you should know

Codex API endpoint delivery replaces the entire provider configuration named `farhelm`, including old environment-key
and header settings. General settings, other providers and comments outside that table survive. Claude API endpoint and
Subscription delivery remove saved custom headers, the saved API key and the root credential helper. The latter two were
an explicitly permitted choice: Claude's current documented precedence puts them above a subscription setup-token. Other
Claude settings survive. Neither agent's existing None-mode cleanup changed.

This is not a complete credential scrub. Credentials elsewhere in an image or host remain subject to the undefined
mixed-ownership rule. A gateway needing extra headers belongs under None, with the image supplying all its credentials.
Existing hosts receive the cleanup only on the next credential delivery, such as an endpoint, token or login change;
upgrading alone does not force redelivery. Both changelog fragments say this.

The code, tests and specification changes count 79 added-plus-deleted lines for Codex and 97 for Claude, excluding
ledger, queue and changelog bookkeeping, within each outcome's approximately 100-line gate. Each feedback file and index
entry is removed, and its decision moved to the archive with its change id, bookmark and PR URL.

# Open questions and possible follow-ups

None required for this plan. The separate Podman plan may later move credential delivery; these changes add no provider
abstraction or dependency on that work. No live installation, cloud resource or real agent credential was used.

# The PRs

- [#3290: clear old Codex endpoint credentials](https://github.com/scode/farhelm/pull/3290/changes) — draft, based on
  main, head `ecc31f19`; shared ownership contract and Codex outcome.
- [#3291: clear old Claude recipe credentials](https://github.com/scode/farhelm/pull/3291/changes) — draft, based on
  #3290, head `57e618f5`; Claude outcome.

# Checks run, reused and skipped

Runtime checks used the recorder, pinned nextest 0.9.148, four slots, zero retries and no tmux. Both ran:

```sh
python3 scripts/record-test-run.py --runner nextest --kind development --tmux none --selection '<recipe endpoint and credential delivery units>' --concurrency '4 nextest slots; retries 0' -- cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::agent_endpoints::/) | test(/^provisioning::credential_delivery::/)'
```

- Codex: 28 passed, run `98ceb28b-bed7-494f-afe9-09e34cb85620`, recorded main `3ee28ff2` plus the Codex draft, source
  fingerprint `e179b5d9964cbc96a68c520640ea67bee469a1d2e792caaaee04e9e6affb0f73`. Later changes were assertion wrapping,
  explanatory specification text and outcome bookkeeping. The executable change remained the tested one.
- Combined fixes: 29 passed, run `1a2eefaa-5435-4e51-bfaf-e0da6c4bb299`, recorded Codex head `ecc31f19` plus the Claude
  draft, source fingerprint `b7995a557c58739f242aff40f1b1966cbbab0dab974c7aa93c45c7e4a34355cd`. The final change after
  that run was a helper docstring correction and outcome bookkeeping; runtime behavior and tests remained unchanged. The
  tests cover Codex's ordinary, inline and nested-header tables; Claude's token-bearing and tokenless endpoints and
  Subscription; None preservation; malformed-file refusals; and delivery/replacement/reconnect behavior.
- Both records and reports are complete, with complete output scans and no runtime `SKIPPED` markers or failures. Each
  selection filtered out 1,648 other tests. The two runs contain 57 passing executions, not 57 distinct tests.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed for each PR. Rust formatting, configured Markdown
  formatting, changelog format, test-sleep checking (429 delays, none unexplained) and diff whitespace checks passed.
  The instruction checker passed with zero errors and one existing archive citation warning. None of these changes adds
  a doctest or generated artifact.
- Latest main `6a847571` differs from the stack's base only in plan bookkeeping and another plan's report. That delta
  was read and has no interaction with credential merging; no rebase or repeated runtime checks were needed. The
  recorded runtime evidence is reused for the final pushed heads for the reasons above.
- Broader Rust, browser, desktop, installer and cloud tests were skipped: the executable change is confined to pure
  settings merges, and the focused merge and delivery tests cover the affected contracts. No GUI or transport changed.

# Review gate's outcome

Each PR passed its required single fresh GPT-6.1 Sol high review with the full goal and test-authoring checklist. Codex
had no substantive findings. Claude had one stale helper comment, which was corrected and accepted on recheck; no
findings remain and none were rejected. Separate fresh readers understood each PR's title and body.

### Landing

Landed alone (the only plan in this landing round), as #3290 then #3291. Since the stack was based, main gained only the
browser terminal's scrollback fix (#3289), which changes none of the code these PRs change. Rebasing onto it conflicted
only in `TRIAGE_OUTCOMES.md` and `TRIAGE_OUTCOMES_ARCHIVE.md`, where both sides had moved finished entries; the landing
kept both sides. The landing made no other change.

#### Review

The landing's fresh-context review found nothing that must change. It ran the Codex settings merge on its own against
ordinary, inline and dotted provider tables, nested header tables, a provider chosen elsewhere, comments and an empty
file, and a Codex profile that selects another provider. Every result was valid, Farhelm's own provider ended up with
exactly its own settings, other providers were left alone, and running the merge again changed nothing. For Claude it
found no way to leave settings half-updated, and repeated deliveries give the same result. SPEC.md and SPEC_impl.md
match the code and each other.

#### Things you should know

From the review. All of these follow from the decision that an agent's credentials come from Farhelm or from the image,
never both, but the report and changelog entries do not spell them out:

- When a recipe gives Codex an API endpoint, Farhelm now replaces its own provider entry (the one named `farhelm`)
  whole, so other settings in that entry go too, not only credentials: extra query parameters (Azure's API version, for
  example), timeouts and retry settings. They could only be there because an image or someone editing the host put them
  in Farhelm's entry; other providers' entries are untouched.
- A Claude API endpoint used without a token, which relied on an API key or credential helper saved in the image's
  Claude settings, loses that credential at the next delivery. This is the one case that can break a setup that works
  today. Such a setup now belongs under None, with the image supplying the endpoint as well. The landing's view is that
  this is the decision working as intended, but the changelog entry does not mention it, and a caveat there would warn
  anyone relying on it.
- An API key kept in Claude's settings for another purpose, such as hooks or tool servers, is removed too.
- The cleanup does not undo itself: switching a host back to None does not bring the image's removed settings back.
- Other places an image can carry credentials are still left alone, as the best-effort rule allows: a Codex profile that
  selects another provider, Claude's cloud-provider switches (Bedrock, Vertex, Foundry), project or managed settings,
  and variables the login shell exports.
- Existing hosts get the cleanup only at their next credential delivery that changes something (an endpoint, token or
  login change), as the report says. A reconnect does go through credential delivery, but when nothing changed since the
  last delivery it skips rewriting the settings, so an upgrade followed by a reconnect does not clean a host on its own.

A possible follow-up: the user documentation page on Tensorlake sandboxes does not yet say that each agent's credentials
come from either Farhelm or the image, nor what Farhelm now removes.

#### Checks

Run now on this development machine, on the rebased stack, whose tree is what merged:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- Recorded `cargo nextest run -p farhelm-helm`, the whole helm crate rather than the two provisioning modules the
  executor ran: 1675 passed, 2 skipped by design (the two tests that need a real remote Mac; run
  `1ad0ebed-f8bf-4a99-8781-cfcd50195136`). Its output has no runtime `SKIPPED` lines, so no test returned early and
  counted as a pass. The changed merges are reached through credential delivery, which host setup, credential
  replacement, reconnects and sandbox sessions all use, and those callers' tests live in the same crate.

Reused: nothing; the run above supersedes the executor's two narrower runs.

Skipped: browser specs (the recipe, Tensorlake host and credential settings specs use a test backend that discards what
credential delivery writes, so they cannot see this change), the rest of the workspace's Rust tests (only the helm
crate's code changed), and hosted CI (manual-only, not dispatched).
