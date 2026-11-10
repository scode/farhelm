## What this was about

Tensorlake snapshot builds and sandbox-host connections could offer a personal key loaded in the user's SSH agent before
Farhelm's registered key, despite `IdentitiesOnly=yes`. If the gateway rejects the first unknown key, that can refuse an
otherwise correctly registered connection. The maintainer chose to stop consulting the agent without first checking the
gateway live.

## Things you should know

The gateway connection now carries `IdentityAgent=none` for builds and hosts. Ordinary SSH hosts keep their existing
authentication policy. Proxy and jump configuration remains available; a jump hop's separate SSH process retains its own
agent use, as the existing localhost observation and OpenSSH's command construction establish.

Execution found that disabling the agent does not guarantee Farhelm's key is always first. A configured certificate with
its matching private key, or a configured hardware-token key, can still precede it. Such a setup could still be refused
by a gateway that rejects the first unknown key. The specifications, both Tensorlake module comments and the fragment
now state those exceptions; the fragment advises leaving those settings out of SSH configuration applying to Tensorlake
hosts. The gateway's first-unknown-key behavior remains unverified live.

The landing monitor accepted this agent-only scope on the maintainer's behalf in the plan's latest Decisions and
recorded it for later reconsideration. That replaces the earlier first-or-only-key completion wording. No generated
configuration, broader identity isolation or client-minimum increase was introduced. The hardware-token override was
omitted under the plan's compatibility fallback: OpenSSH 7.6 supports `IdentityAgent=none`, but passes
`PKCS11Provider=none` to its library loader. See the
[7.6 agent option](https://github.com/openssh/openssh-portable/blob/V_7_6_P1/ssh_config.5#L845-L855) and
[provider loader](https://github.com/openssh/openssh-portable/blob/V_7_6_P1/ssh-pkcs11.c#L540-L561).

The counted source, test and specification diff is 82 added/deleted lines. The agreed outcome is archived with its
narrowed criteria recorded in Execution; its feedback file and index entry are removed in the same PR.

## Open questions and possible follow-ups

No unanswered execution decision remains. Broader isolation that guarantees the first key in every SSH configuration is
a future policy choice recorded for the maintainer, rather than part of this fix. No live gateway check was added.

## The PRs

[PR #3300](https://github.com/scode/farhelm/pull/3300/changes) — stop using the SSH agent for Tensorlake; draft head
`f1ca43930e1e777fafef831eb89af5c44e6694eb`.

## Checks run, reused and skipped

Run on the final documentation and bookkeeping:

- `cargo fmt --all -- --check`, targeted `dprint check` of changed Markdown, and
  `python3 releasing/check-changelog.py format` passed.
- `python -B scripts/check-test-sleeps.py` with the isolated parser interpreter passed: 429 delays, zero unannotated.
- `python3 scripts/check-agents-md.py` passed with zero errors and one existing archive-citation warning, checking
  references affected by the feedback removal.

Reused from the partial implementation:

- Recorded `cargo nextest run -p farhelm-helm --lib -E <filter>` selected the three `tensorlake::ssh::` tests, shared
  Tensorlake argument construction, transport dialing and shared-master release: six passed, 1671 outside the selection.
  Run `99bb1985-c3b7-480d-816f-0f30012ea83b` used nextest 0.9.148, four slots, zero retries and the required pinned
  tmux. Source identity, JUnit and retained output are complete, with no runtime `SKIPPED` lines, omitted output,
  truncation or forced cleanup.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed on that implementation.

Those checks covered base `fc454545` plus the partial working diff, with recorded source fingerprint
`2d12f6e14d5014296bcc9c9b594e2e066d9ceabdd068a65ba248fc76461aca10`. The rebase onto `820b2536` preserved the patch
exactly. Upstream's Mac stat and tmux cleanup changes do not affect the Tensorlake options or their callers; its other
changes were bookkeeping. This round changed only documentation and outcome bookkeeping, so the runtime and Clippy
coverage still applies.

Skipped: new runtime runs, full workspace, browser, desktop and cloud tests, because no further executable behavior or
UI changed. No Tensorlake account or live gateway experiment was used. Hosted CI is manual-only and was not dispatched.
The live installation was not used.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed the full final candidate under the latest
Decisions with no actionable findings. No finding was rejected. A separate native resume check reconciled the earlier
draft, and a blind commit-message reader found the final explanation consistent with the change.

### Landing

Landed alone (the only plan in this landing round), as #3300, after the landing monitor answered its block on the
maintainer's behalf (accept the agent-only fix and document the exceptions; see its Decisions). Since these changes were
written, main gained the Hosts panel's Suspend fix (#3299) and a revision of another plan's file; neither touches SSH.
Rebasing conflicted only in the triage records, where both sides had moved entries; the landing kept both.

#### Fix made by the landing

The code comments, SPEC.md, SPEC_impl.md and the changelog entry said a configured certificate "with its matching
private key" could still be offered before Farhelm's key, and so does the report's "Things you should know". The review
found that OpenSSH offers any configured certificate first, private key on disk or not. That includes a common setup, a
certificate configured for every host with its private key held only in the agent, whose owner would have read the
caveat as not applying to them. The landing dropped the qualifier and said so explicitly in the module comment of the
Tensorlake SSH command code, SPEC.md, SPEC_impl.md and the changelog entry; the monitor's own answer to the block had
used the same wording. It missed the second Tensorlake module comment (the one on Farhelm's Tensorlake key), which still
has the qualifier; that is a one-line follow-up. This supersedes the report's statement: any configured certificate can
be offered first, whether or not its private key is on disk. Nothing executable changed.

#### Review

The landing's fresh-context review found nothing else that must change. OpenSSH 7.6, Farhelm's minimum, supports
`IdentityAgent=none`, the option that turns the agent off, and a client too old for it already could not connect,
because Tensorlake connections need 7.6 for another option. macOS's `ssh` is the same code. The option is set in one
shared place used by snapshot builds, snapshot preparation, setting up a Tensorlake host, sending Farhelm's agent
credentials to it and connecting to it, and ordinary SSH hosts get nothing new (a test checks that). A jump host in the
user's own SSH configuration still gets the user's agent. Nothing else relied on the agent: Farhelm's key has no
passphrase, and agent forwarding was already off.

#### Things you should know

- Someone whose SSH configuration names, for the Tensorlake host, a personal key they registered with Tensorlake and
  hold only in their SSH agent (instead of Farhelm's key) now has Tensorlake builds and connections fail. That is the
  intended effect, but the changelog entry does not say so; release curation may want a sentence.
- SPEC.md's general provisioning bullet still says provisioning uses the user's "keys, agent, and config" without
  excepting Tensorlake, although the new paragraph a few lines above does make the exception. SPEC_impl.md's later list
  of Tensorlake SSH options also leaves out the new one, and one of its new sentences reads as change history ("for this
  change"). Small wording follow-ups.

#### Checks

Run now on this development machine (the two test runs through the recorder with the pinned tmux first on `PATH`;
neither printed a runtime `SKIPPED` line):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^tensorlake::/) | test(/^provisioning::/)'`: 482 passed (run
  `e57c3337-15c8-4bac-9995-73668a78b3cb`), on the stack before the landing's wording fix.
- After that fix, which changed only comments, SPEC.md, SPEC_impl.md and the changelog fragment: every helm caller that
  passes the Tensorlake SSH options through (`cargo nextest run -p farhelm-helm --lib -E <filter>`, the filter selecting
  the `tensorlake::`, `ssh::` and `transport::` modules and the provisioning modules `backend`, `snapshot_preparer`,
  `tensorlake_backend`, `tensorlake_add` and `credential_delivery`): 202 passed (run
  `cc32b18f-3756-4c00-a5a7-fdec39ebff4a`). It is narrower than the first run because the fix touched only comments and
  documents. The tree it tested is the one that merged.

Reused from the executor: nothing; the runs above include its selection, and clippy was run again.

Skipped: browser specs (the Tensorlake specs use a stand-in API and shell and never run `ssh` to the gateway), a live
Tensorlake gateway (whether it rejects the first unknown key stays unverified, as the plan's decision accepts), the rest
of the workspace's Rust tests (only the helm's Tensorlake SSH options changed), and hosted CI (manual-only, not
dispatched).
