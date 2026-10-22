# A devbuild's Update can leave a shared remote host on a database the installed app's build refuses

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When the maintainer tries a change with the devbuild flow and presses Update on a remote host, the change's supervisor
replaces the one the installed Farhelm also uses on that host. If the change adds a supervisor database migration, the
host's real session database is migrated forward. The installed app then shows the host as an older version and offers
Update, and the devbuild instructions present that as the way back. Pressing it installs a released supervisor that
refuses the migrated database, so the host becomes unreachable from the installed app until the dev build is put back by
hand. Its agents keep running. In a rarer case, where two unlanded changes each add a migration with the same number, or
the change's migration later lands renumbered or reshaped, a later supervisor can misread that database instead of
refusing it.

## Details

Found by the 2026-10-22 prioritized persistent review, group `216-maint-scripts-4of9` (finding f1).

Mechanism:

- `scripts/devbuild-mac.sh:498-537` stages Linux `farhelm` payloads built from the working copy into
  `target/devbuild/payloads`, and `launch` (`:304-315`) points the dev app's embedded helm at them through
  `FARHELM_HELM_PAYLOAD_DIR`. So the dev helm's Update installs that build on any remote host added to it. The header
  (`:150-152`) says remote hosts are shared with the installed app by design: "Update on a host replaces the `farhelm`
  its supervisor runs, whichever app drives it."
- The supervisor refuses a database whose `user_version` is above its own `SCHEMA_VERSION`
  (`crates/farhelm-supervisor/src/store.rs:1391-1405`). `crates/farhelm-helm/src/provisioning/service.rs:238-258`
  describes the consequence of a downgrade: "an older supervisor refuses a database written with a newer schema, its
  unit restarts on the failure, and the host stays unreachable until the newer build is reinstalled by hand."
- The dev build reports `0.0.0-unreleased` (`scripts/devbuild-mac.sh:6`). `is_development_build`
  (`crates/farhelm-proto/src/version.rs:12-15`) recognises it, and `build_is_older`
  (`crates/farhelm-helm/src/hosts.rs:353-355`) ranks it below every release, so the installed app labels the host as
  older and offers Update. `build_is_newer` (`hosts.rs:383-385`) special-cases a development build only on the helm's
  own side, not the host's, so `downgrade_refusal` (`service.rs:250-258`) lets the Update through. The downgrade guard
  added for `TRIAGE_OUTCOMES.md` "update-silently-downgrades-newer-hosts.md" does not fire. No schema-version check
  exists in helm provisioning or the protocol crate (searched `crates/farhelm-helm/src/provisioning`,
  `crates/farhelm-helm/src/provisioning.rs` and `crates/farhelm-proto/src`).
- `.agents/devbuild.md:29-34` says the installed app "offers to update it back", which reads as the remedy. Its schema
  NOTE (`:36-45`) covers only the local devbuild state, and the per-target `FARHELM_DESKTOP_STATE_DIR` variant it offers
  does nothing for remote hosts. Neither the script nor the doc warns that a target that changes the supervisor schema
  must not be pushed to a host the installed app uses, or that updating back fails afterwards.

Scenario: the maintainer runs the devbuild line for a PR that adds supervisor migration N+1, adds a real remote host to
the devbuild, and presses Update. Later, in the installed app, they press the offered Update. The released supervisor
refuses `user_version` N+1 and crash-loops, and the installed app cannot reach the host. In the worse variant, two
unlanded PRs each add a migration numbered N+1 with different meaning, or the PR's migration lands renumbered, and the
later supervisor treats the database as already at N+1 and misreads it. `.agents/devbuild.md:36-38` names the same
misreading for the local state.

How to verify: read the migration ladder in `store.rs` and the ordering in `hosts.rs`. On a test host, install a build
with a bumped `SCHEMA_VERSION` from a payload directory, then press Update from a release-versioned helm and watch the
supervisor refuse the database.

Fix sketch: have the dev helm, or the payload staging in `devbuild-mac.sh`, refuse or loudly confirm pushing a build
whose supervisor `SCHEMA_VERSION` is above the latest release's. Or have `.agents/devbuild.md` and the script header say
that a schema-changing target must not be pushed to hosts the installed app uses, and that updating back does not work
after it. A helm-side guard could compare the host's reported schema version before Update, if one were exposed.

Suggested bucket: the reviewer said high. The host is not lost, its agents keep running, and reinstalling the dev build
makes it reachable again. It would be highest if triage counts the migration-number collision case, which can misread,
in effect corrupt, a supervisor database on a real host. The harm comes through maintainer tooling but lands on the
maintainer's own remote hosts. Recorded under high; triage settles the bucket.

Confidence: confirmed by code reading, not reproduced.

Reviewer's proposed drop, kept for triage: possibly maintainer tooling by design (`devbuild-mac.sh:150-152`, quoted
above). The reviewer noted that this accepts sharing the binary, not leaving the host's database unopenable for the
installed app, while `.agents/devbuild.md` presents updating back as safe.

Possible cover: `.agents/devbuild.md`'s NOTE on schema changes (local state only); `TRIAGE_OUTCOMES.md`
"update-silently-downgrades-newer-hosts.md" (fixed the ordered-release case; its assessment named "switching between
RC/dev and stable helms" as a trigger, but the fix leaves a development-build host ordered as older). FILTER.md
"Upgrades from older releases combined with another rare condition" does not apply: this is not an upgrade from an older
release, and that filter excludes a corrupted supervisor database. `TRIAGE_OUTCOMES.md`
"devbuild-git-environment-discards-caller-work.md" discarded a different devbuild finding as harm confined to maintainer
tooling under a deliberately set environment; here the trigger is the devbuild flow's ordinary use.
