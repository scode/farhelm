# A paused trust writer can publish after another writer takes its lock

Reviewed commit: 0ad806334217ba8dde4e7f7dc6ecc7480d27cb9a

## TLDR

If Farhelm pauses after its final safety checks, it may later overwrite newer Claude configuration after Claude has
taken over the expired lock.

## Details

Suggested bucket: `highest`. Confidence: likely; Claude 2.1.300's actual stale-lock takeover and publication while
Farhelm alone is suspended were not reproduced. Loss of unrelated credentials, settings, or trust updates is the
potential consequence, not an observed incident. Source: Claude trust adversarial reviewer, followed by a fresh
independent exclusion review. Proposed drop: rejected; retain the unverified vendor and scheduling premises.

`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:276–282` checks the lock's age and identity, checks the target
inode, and then renames its whole-document snapshot over the target. If the process is suspended after those checks but
before rename, the vendor's stale-lock interval can elapse. A locking Claude writer could replace the expired lock and
publish new configuration before Farhelm resumes and overwrites it with its older snapshot. No malicious mutation of
managed checkout paths is required for this scheduling scenario.

Possible cover: `SPEC.md:827–829` and `SPEC_impl.md:1881–1883` accept Claude's unlocked exit-save race and future vendor
changes. The first reviewer proposed that acceptance or deliberate-interference limits as grounds to omit this case. The
independent reviewer rejected that exclusion: a current locking writer taking over after scheduling suspension is a
different trigger, and the rare-glitch filter excludes credential/data loss. These passages remain possible cover for
triage; they do not clearly settle the reported scenario.

Verify by pausing the Farhelm writer immediately after its final inode comparison, observing Claude's stale-lock
takeover and a changed unrelated value, then resuming Farhelm and checking that value. The independent review inspected
the relevant implementation and specifications but did not inspect the vendor binary or run this reproduction. Any fix
must account for the vendor's lock protocol rather than treating another pre-rename age check as an atomic guarantee.
