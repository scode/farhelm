# The CentOS test changes the meaning of existing global SSH settings

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The CentOS test changes the scope of existing SSH settings.

## Details

F116 — **definite** — `scripts/test-provision-centos.sh:378–379`; `scripts/test-provision-centos.sh:379` — The CentOS
test changes the meaning of existing global SSH settings

Prepending the fixture's Host stanza leaves that section active when the original SSH configuration begins. Leading
directives that were global become conditional on the fixture alias; comments and blank lines do not restore scope.
Other connections can lose their intended user, identity, or routing settings during the test and longer after
interruption. Restore global parsing scope inside the removable block before appending the original configuration.

## Evidence and triage context

- scripts/test-provision-centos.sh:79 selects the user's real SSH config. Lines 367–378 write one Host block and its
  comment marker; :379 appends the original file with no intervening Host * reset; :381 publishes it. Unrelated SSH
  invocations during the test therefore no longer receive formerly global leading options.
- scripts/test-provision-centos.sh:79 selects the user's actual SSH configuration.
- scripts/test-provision-centos.sh:366-381 prepends Host and fixture options, then only a blank line and comment before
  appending the original configuration.
- scripts/test-provision-centos.sh:395 and 419-426 use the alias during readiness and the provisioning test, leaving the
  altered configuration installed throughout.
- /usr/share/man/man5/ssh_config.5.gz:59-64 states that Host separates conditional sections; lines 97-100 identify blank
  and comment lines as comments.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC_impl.md:3590-3601 authorizes using an alias in the user's SSH configuration, but does not accept changing the
  meaning of unrelated directives. BUGS.md:80-101 concerns inherited SSH output pipes, not configuration scope.

Caveats:

- Requires leading global directives before the original first Host or Match. Existing authenticated connections may
  remain unaffected.
- Requires original directives before the first explicit Host or Match boundary.
- Normal successful teardown restores the original scope.
- No SSH connections or runtime tests were performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F1`,
`automation_website_10_sec:p1:F2`.

- `automation_website_10_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
- `automation_website_10_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: high.
