# Release brick protection: the overall plan

NOTE: This is a high-level plan, written before any of it was built. It records where things stood on 2026-10-05, what
the maintainer decided, and the order the work was meant to happen in. It is not a spec, and the individual pieces are
planned and specified separately as they are picked up.

## The goal

Get as close as possible to zero chance of shipping a release that bricks an existing installation: an app that will
not start, a helm that will not come up on its old state, or a helm that cannot update the supervisors an older release
installed. Until now the maintainer caught these by hand, installing RC releases on real machines. The 2026-09-01 field
failure (a protocol-12 host that a protocol-14 helm could not update, because the probe misread the version-skew refusal
as a transport failure) is the worked example of the class: it shipped because nothing anywhere ran an old release next
to a new one.

The priority is brick protection per unit of effort, soonest. The maintainer explicitly accepted that, for a few
releases, the first line of defence may be a slow test that fails late and forces a fix, as long as it fails before the
release reaches users. Making failures surface sooner and cheaper is the second phase, not the first.

## Where things stood

Facts as of this date, from reading the release workflow and the tests:

- No test anywhere involved a previously released build. Every update test either updated a build to itself (the
  CentOS provisioning leg "updates" to the same binary with bytes appended), or used stand-ins: fake two-line shell
  scripts for the installer tests, a hand-built old-protocol hello for the skew tests, frozen schema snapshots for the
  database migrations.
- The release gate tested separately built debug binaries, compiled in the same CI job, not the bytes that ship. The
  published files were exercised only after the GitHub release was already public, by one job that runs the published
  x86_64 Linux `farhelm --version` and starts a helm. The published macOS, arm64 Linux and desktop binaries were never
  run.
- An RC and the stable release after it were separate builds from separate release commits, since the version is
  compiled in. Testing an RC by hand therefore never tested the stable bytes.
- The in-app updater only ever asks `get.farhelm.io/latest`, and its TLS uses webpki roots, so a test machine cannot
  redirect it by trusting a local CA. The helm downloads remote-host payloads for its own version from
  `get.farhelm.io/v<version>/`, and the installer takes `FARHELM_VERSION`, so both already work against a staged,
  signed release that `/latest` does not name yet.
- One suspected brick path, inferred from code and not verified: a new desktop app that finds an old local supervisor
  still answering reuses it, and the local host then shows "needs update" with its Update action greyed out.

## Decisions

**No version override for testing.** The binary built from a release tag already carries its final version, so a test
does not need a differently versioned build. What a test changes is only where the files come from. That is what makes
"the tested bytes are the shipped bytes" achievable without reproducible builds.

**Release kinds: dev, unstable, stable.** RC releases go away. What an RC was used for becomes a dev release: a
low-effort, agent-driven release the maintainer installs to try a build on a real machine, close to a real release
rather than an rsync-and-cargo-build setup. Dev releases are not candidates, are not signed, and are not for ordinary
users. A future unstable channel would be signed; it is out of scope for now. A stable tag is itself the candidate, and
a candidate that fails testing is never promoted and is replaced by the next patch version, as a failed tag build
already is.

**Sign, stage, test, promote.** A stable candidate is signed and published at `get.farhelm.io/v<version>/` without
moving `/latest`, tested there, and promoted by moving `/latest`. A signed release that never gets promoted is
acceptable. The maintainer may revisit this if Farhelm gets a non-trivial number of real users.

**Upgrade epochs, later.** The intended long-term support rule: releases carry an upgrade epoch, and an installation
only has to upgrade cleanly across one epoch. A change that needs new logic in the upgrade path ships it in a release
that starts a new epoch; once that release has been out for a while (a week or a month), the next epoch can rely on it.
An installation two epochs behind is told to uninstall and reinstall. The details are deferred to a TODO entry. Until
that lands, SPEC.md forbids changes that break upgrading the desktop app, the helm or a supervisor.

**The Mac test machine.** The maintainer runs a Tart VM from a clean-ish macOS image, with an agent inside it that has
computer use, signed in to real Claude and Codex accounts. The remote host it adds should look like any other ssh host
to Farhelm. A container inside the macOS VM was the maintainer's first idea. The decision is a separate Linux Tart VM
on the same Mac instead: it is simpler and closer to a real remote host. Containers inside a macOS VM need nested
virtualization (as far as I know, only on M3 or newer hosts, and the test Mac at the time was an M2 Max), and a VM has a
real systemd user manager, which the remote supervisor's service depends on, where a container needs special setup to
provide one.

## The work, in priority order

1. **Agent-driven Mac release test, in the Tart VM.** The highest return, and first. It is the only thing that can
   exercise the path most users take (the Mac app, its in-app updater, the helm inside it, and the remote hosts that
   helm updates) with real agents and real state. It is a standing recipe in the repository, plus a short per-release
   addendum drafted from that release's changelog fragments, naming what changed and deserves hands-on checking. Each
   run starts from fresh clones of the golden images. The standing part covers a fresh install through the public
   installer; an upgrade from the previous stable release with real state built up first (a remote host, sessions with
   real agents, templates, settings), through "Restart to update", checking that sessions survive, the local host comes
   back at the new version, and the remote host updates and keeps its sessions; and a quit and reopen at the end. The
   run returns screenshots, logs and a structured pass or fail report.

   The first slice is built in two halves. An unattended executor on Linux, which cannot run Tart, writes the recipe,
   a bring-up document, and the updater override below. The bring-up document says what is being built and what is
   left, with recommendations rather than settled details (which Linux image, how the golden images are made, how the
   VMs find each other), for an agent on the host Mac to resolve in the real environment, asking the maintainer where
   it must, and land as its own PR. That agent is pointed at the repository with one sentence; no separate hand-off is
   written outside the repository.

   The in-app update click can only target an unpromoted candidate if the old app can be told which version counts as
   latest. A test-only override (signatures still checked, so it can only ever name a signed release) helps only once a
   release carrying it is the old side of the test, so it should ship early. Until then the recipe runs the
   candidate's installer by hand while the old app runs, which is what the updater does under the hood, then quits and
   reopens the app.

2. **Run the published artifacts before signing.** Start every published binary (x86_64 and arm64 Linux, the macOS CLI
   and desktop app on a macOS runner) far enough to prove it comes up. Cheap, and it catches "will not start" without
   the VM.

3. **Remote-host upgrade from real old releases, in Linux CI.** A CentOS-leg variant: provision a container with the
   previous stable release's helm and payloads (downloaded and signature-checked from get.farhelm.io), give it real
   state with fake-agent sessions, start the candidate helm on the same state directory (which also exercises the helm
   database migration), check that the probe sees a host needing an update, run the update, and assert the supervisor
   comes back at the new version with its tmux sessions intact and old-launched sessions still working, including
   restart and resume. Checked once against a release from the 2026-09-01 era to prove it would have caught that bug.

4. **Headless Mac upgrade on a hosted macOS runner.** Install the previous stable release with the real installer under
   a temporary HOME, run its supervisor and seed sessions, install the candidate over it, and check that the new
   supervisor takes over the old state. Everything except the GUI clicks, which stay in the VM.

5. **Release flow changes.** Drop RCs in favour of dev releases, and add the stage-then-promote split for stable.

6. **Upgrade epochs.** Once in place, the SPEC.md freeze on upgrade-breaking changes is replaced by the epoch rule.

Phases 2 to 4 are the "fail sooner" phase: each moves part of what the VM run catches into something cheaper and
earlier. The suspected greyed-out local Update deserves a look whenever item 1 or 4 is built, since either would show it.
