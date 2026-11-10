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
