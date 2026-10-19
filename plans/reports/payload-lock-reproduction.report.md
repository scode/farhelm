# Helm release-cache overlap investigation

## What this was about

A helm can stop serving and release its state-folder lock while old background work is still unpacking the Farhelm
program files it installs on hosts. A replacement helm can then claim that folder and write the same cache. The concern
was that a later installation might use bytes the cache calls authentic even though they differ from the signed release.
The realistic trigger in the finding was a desktop startup failing after its built-in helm became ready during host
provisioning, followed by another helm starting. No corrupt installation had been demonstrated.

During triage, the maintainer retained the rule of discarding rare-trigger findings without shown damage unless a fix is
very cheap, but asked for this finding to be reproduced before deciding. This plan ran the requested three schedules and
recorded the evidence. A later triage will decide what to do with the finding; its feedback file remains in the queue.

## Things you should know

The ownership overlap reproduced. In each schedule, a real embedded helm became ready, an independently started
extraction worker was parked, and the helm returned from serving. A successor acquired and held the actual state-folder
lock while that worker was still parked. A second independent source then finished publishing into the same cache before
the old worker resumed.

| Where the old worker paused          | Old worker after the successor finished             | Later independent lookup                                                |
| ------------------------------------ | --------------------------------------------------- | ----------------------------------------------------------------------- |
| After writing the staged executable  | Failed opening the removed staging file for hashing | Cache hit; no network requests; bytes matched the signed archive member |
| After hashing the staged executable  | Failed renaming the removed staging file into place | Cache hit; no network requests; bytes matched the signed archive member |
| After publishing its checksum marker | Failed renaming the removed staging file into place | Cache hit; no network requests; bytes matched the signed archive member |

No incorrect cache hit was observed in these three forced schedules. The old worker's filesystem errors were expected
observations inside a passing reproduction test. They are consistent with an old provisioning attempt failing while the
replacement's cached files remain intact. The test did not run a complete host provisioning attempt, so it does not
establish the final provisioning error a user would see.

The reproduction used the real signature-verification, extraction and cache-publication code with a small signed test
release. It verified the staged bytes before the overlap and the final returned bytes against that fixture's signed
archive member. It used isolated state and a loopback fixture server; it did not touch an installed helm or supervisor.
The extraction source was called directly, rather than through an accepted host-provisioning request. Serving ended
through the embedded helm's shutdown signal after readiness. Full desktop startup failure and runtime destruction were
not reproduced. Three schedules do not establish safety for every interleaving, particularly late opens or truncation at
other boundaries or asynchronous download work.

The investigation ran against main `41e332436529d88d932324dcde0ef08172b0a535` plus the scratch patch below. The
ledger-only PR was then rebased onto `99443076d9a6be02d552714f5ccace7fa2516ca4`. The intervening work changed
agent-prompt admission and separate specification paragraphs; it did not change cache publication or state ownership.
The ledger continues to name the actual investigated revision.

The patch below records the exact scratch hooks and test. It was kept on an isolated, unbookmarked change and is not
part of the PR or the product. The scratch change is abandoned when this plan closes; this report preserves the patch.

<details>
<summary>Scratch reproduction patch</summary>

```diff
diff --git a/crates/farhelm-helm/src/provisioning/release_payloads.rs b/crates/farhelm-helm/src/provisioning/release_payloads.rs
index 1a46e5407d..93ab6bc5ea 100644
--- a/crates/farhelm-helm/src/provisioning/release_payloads.rs
+++ b/crates/farhelm-helm/src/provisioning/release_payloads.rs
@@ -292,6 +292,10 @@ pub(super) struct ReleasePayloadSource {
     /// the map lookup (never held across an await); the value it hands out
     /// is the async mutex actually held for the duration of a download.
     asset_locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
+    /// Scratch-only observation of an old publisher's filesystem boundaries.
+    /// Per source, so a successor never shares the pause or any global state.
+    #[cfg(test)]
+    publication_pause: Option<Arc<dyn Fn(&str) + Send + Sync>>,
 }
 
 /// The parsed, signature-verified contents of one release's `SHA256SUMS`:
@@ -389,6 +393,8 @@ pub(super) fn with_transport(
             sums: OnceCell::new(),
             housekept: OnceCell::new(),
             asset_locks: std::sync::Mutex::new(HashMap::new()),
+            #[cfg(test)]
+            publication_pause: None,
         }
     }
 
@@ -844,6 +850,8 @@ async fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<
         let staged = self.cache_dir.join(format!("{asset}.bin.part"));
         let marker = self.cache_dir.join(format!("{asset}.bin.sha256"));
         let published = binary.clone();
+        #[cfg(test)]
+        let publication_pause = self.publication_pause.clone();
         tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
             match member {
                 Some(member) => extract_single_member(&downloaded, member, &staged)?,
@@ -853,13 +861,25 @@ async fn path(&self, payload: PayloadKind, arch: PayloadArch) -> anyhow::Result<
                 // payload kinds.
                 None => copy_executable(&downloaded, &staged)?,
             }
+            #[cfg(test)]
+            if let Some(pause) = &publication_pause {
+                pause("after-staging");
+            }
             let digest = sha256_file(&staged)?;
+            #[cfg(test)]
+            if let Some(pause) = &publication_pause {
+                pause("after-hashing");
+            }
             // Marker first, then the rename: a crash between the two leaves
             // a marker with no `.bin`, which the cache check reads as a
             // miss. The reverse order would leave a `.bin` with no marker,
             // which is the same miss but one that has already published a
             // file under its final name.
             publish_bytes(&marker, format!("{digest}\n").as_bytes())?;
+            #[cfg(test)]
+            if let Some(pause) = &publication_pause {
+                pause("after-marker");
+            }
             std::fs::rename(&staged, &published)
                 .with_context(|| format!("installing {}", published.display()))
         })
@@ -1804,6 +1824,179 @@ mod tests {
     use super::*;
     use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
 
+    /// Release a parked blocking worker even when a premise assertion fails.
+    /// Otherwise runtime teardown would wait forever and hide the failed check.
+    struct ReproRelease(Option<std::sync::mpsc::Sender<()>>);
+
+    impl Drop for ReproRelease {
+        fn drop(&mut self) {
+            if let Some(sender) = self.0.take() {
+                let _ = sender.send(());
+            }
+        }
+    }
+
+    /// Observe independent old/new cache writers at each publication boundary.
+    /// Both use the real signed fixture archive; no bytes are injected into the
+    /// cache. A final offline hit must still return that archive member's bytes.
+    /// This investigation records the resulting old-worker failure rather than
+    /// claiming that three schedules exhaust every possible concurrency race.
+    #[farhelm_testtrace::test]
+    async fn repro_payload_publication_after_successor_finishes() {
+        use std::os::unix::fs::FileTypeExt as _;
+        let release = FixtureRelease::start(Vec::new()).await;
+        let expected = expected_member("farhelm", "x86_64-unknown-linux-musl");
+        let asset = assets::archive_name(assets::farhelm_archive_for(PayloadArch::X86_64));
+        for point in ["after-staging", "after-hashing", "after-marker"] {
+            let state = tempfile::tempdir().unwrap();
+            let cache = state.path().join("payloads");
+            let empty_payloads = state.path().join("empty-payloads");
+            std::fs::create_dir(&empty_payloads).unwrap();
+            let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
+            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
+            // Run the real serving lifetime on private state. Its local actor
+            // only probes that directory's absent supervisor socket. The source
+            // below is launched directly, so this is not an SSH/provisioning or
+            // desktop-startup fixture; it tests serving return and extraction.
+            let helm = tokio::spawn(crate::run_embedded(
+                crate::HelmArgs {
+                    port: 0,
+                    state_dir: Some(state.path().to_path_buf()),
+                    ui_dist: None,
+                    ensure_hosts: None,
+                    payload_dir: Some(empty_payloads),
+                    release_base_url: None,
+                    backstop_refresh_secs: None,
+                },
+                None,
+                None,
+                None,
+                ready_sender,
+                shutdown_receiver,
+            ));
+            let _ready = tokio::task::spawn_blocking(move || {
+                ready_receiver.recv_timeout(std::time::Duration::from_secs(15))
+            })
+            .await
+            .unwrap()
+            .expect("real embedded serving reached readiness");
+            assert!(
+                std::fs::metadata(state.path().join("helm-token.sock"))
+                    .unwrap()
+                    .file_type()
+                    .is_socket(),
+                "premise: serving owns its control endpoint"
+            );
+            assert!(
+                !helm.is_finished(),
+                "premise: embedded serving remains live"
+            );
+            let reached = Arc::new(tokio::sync::Notify::new());
+            let (sender, receiver) = std::sync::mpsc::channel();
+            let release_old = ReproRelease(Some(sender));
+            let receiver = std::sync::Mutex::new(receiver);
+            let mut old_source = release.source(&cache);
+            let observed = Arc::clone(&reached);
+            old_source.publication_pause = Some(Arc::new(move |boundary| {
+                if boundary == point {
+                    observed.notify_one();
+                    receiver
+                        .lock()
+                        .unwrap()
+                        .recv_timeout(std::time::Duration::from_secs(30))
+                        .expect("the owned test must release its parked publisher");
+                }
+            }));
+            let old = tokio::spawn(async move {
+                old_source
+                    .path(PayloadKind::Farhelm, PayloadArch::X86_64)
+                    .await
+            });
+            tokio::time::timeout(std::time::Duration::from_secs(15), reached.notified())
+                .await
+                .expect("the old publisher reached the selected boundary");
+            assert!(
+                !old.is_finished(),
+                "premise: old publication remains parked"
+            );
+            let generation = generation(&cache, &release.base_url);
+            let staged = generation.join(format!("{asset}.bin.part"));
+            assert_eq!(
+                std::fs::read(&staged).unwrap(),
+                expected,
+                "premise: the parked writer staged the authenticated member"
+            );
+            if point == "after-marker" {
+                assert!(generation.join(format!("{asset}.bin.sha256")).is_file());
+            }
+
+            shutdown_sender.send(()).unwrap();
+            tokio::time::timeout(std::time::Duration::from_secs(15), helm)
+                .await
+                .expect("real embedded serving returned while extraction was parked")
+                .unwrap()
+                .unwrap();
+            assert!(!old.is_finished(), "old extraction outlived serving return");
+            let _successor_ownership = crate::token_control::claim_serving_ownership(state.path())
+                .await
+                .expect("successor claims the state directory before old extraction resumes");
+            assert!(
+                !old.is_finished(),
+                "state reacquisition did not release old work"
+            );
+
+            let successor = release.source(&cache);
+            let published = tokio::time::timeout(
+                std::time::Duration::from_secs(15),
+                successor.path(PayloadKind::Farhelm, PayloadArch::X86_64),
+            )
+            .await
+            .expect("successor publication completed while old worker was parked")
+            .unwrap();
+            assert_eq!(std::fs::read(&published).unwrap(), expected);
+            assert!(
+                !old.is_finished(),
+                "premise: successor did not release the old pause"
+            );
+            assert!(
+                !staged.exists(),
+                "successor consumed the shared staging pathname"
+            );
+            drop(release_old);
+            let old_result = tokio::time::timeout(std::time::Duration::from_secs(15), old)
+                .await
+                .expect("released old publisher finishes")
+                .unwrap();
+            assert!(
+                old_result.is_err(),
+                "old publication unexpectedly succeeded: {point}"
+            );
+
+            let lookup = release.source(&cache);
+            let request_count = release.requests().len();
+            let hit = lookup.cached(&asset).await.unwrap();
+            assert!(hit, "later source recognizes an intact cache entry");
+            let later = lookup
+                .path(PayloadKind::Farhelm, PayloadArch::X86_64)
+                .await
+                .unwrap();
+            assert_eq!(
+                release.requests().len(),
+                request_count,
+                "later lookup is network-free"
+            );
+            let bytes_match = std::fs::read(later).unwrap() == expected;
+            println!(
+                "REPRO point={point} old_error={:#} successor_signed_bytes=true later_cache_hit={hit} later_signed_bytes={bytes_match}",
+                old_result.unwrap_err()
+            );
+            assert!(
+                bytes_match,
+                "a cache hit must equal the signed archive member"
+            );
+        }
+    }
+
     fn mode(path: &Path) -> u32 {
         use std::os::unix::fs::PermissionsExt as _;
         std::fs::metadata(path).unwrap().permissions().mode() & 0o777
```

</details>

## Open questions and possible follow-ups

The finding awaits a new decision. Discarding it would accept the confirmed ownership overlap and the remaining
unexplored schedules under the rare-trigger rule, with these three observations showing safe old-worker failures. Fixing
it would require choosing how to keep cache writes exclusive: retain ownership until writers finish, or give old writers
paths a successor cannot reuse. Those approaches add shutdown coordination or cache-path lifecycle work; this
investigation did not design or establish the cost of either fix. Further investigation could exercise accepted
provisioning through desktop startup failure/runtime teardown, or force late-open/truncate and download schedules. That
would improve coverage but would still need a defined stopping point rather than treating repeated passes as a proof.

The evidence supports retaining the narrow conclusion above for that decision. No fix, discard, or broader race search
was made part of this plan.

## The PRs

- [#2356: record helm release cache race evidence](https://github.com/scode/farhelm/pull/2356/changes) — one draft PR
  updating only this finding's Execution field; the feedback file and its index entry stay.

## Checks run, reused and skipped

- Ran the focused reproduction through the test recorder:
  `cargo nextest run -p farhelm-helm --lib -E 'test(=provisioning::release_payloads::tests::repro_payload_publication_after_successor_finishes)'`.
  Run `33f2852b-5691-4c8e-a70d-089d4669ecf0`, kind `repetition`, four global nextest slots, zero retries, no tmux
  required. One selected test passed, containing all three forced schedules; 1207 tests were selected out. This was one
  attempt. Complete source identity, output, JUnit and successful child/recorder exit were inspected, with no forced
  cleanup or runtime skip. The source fingerprint was `d1e1e237a6c00e3da138d89c35cee5e7536c333fcda3227f0622bbaabe9f31d1`
  against the revision and patch above.
- Ran `cargo fmt --package farhelm-helm` on the scratch source and the isolated test-sleep checker over that workspace:
  304 delays checked, none missing annotations. These inspect the scratch reproduction, which will not ship.
- Ran `dprint check TRIAGE_OUTCOMES.md` for the ledger PR, including after the rebase and PR-link amendment.
- Reused the recorded investigation for its explicitly named revision. The later rebase changed only where the Markdown
  evidence lands; it did not claim to rerun the investigation against the new main.
- Skipped additional repetitions because the readiness gates force the three requested schedules; repeats would not add
  a schedule. Skipped broad Rust tests, Clippy, browser, desktop, installer and provisioning suites for the final PR
  because it changes only Markdown bookkeeping. Full desktop-failure/provisioning reproduction remains a coverage limit
  rather than a passed check.

## Review gate outcome

A single fresh-context gpt-6.1-sol reviewer at high effort reviewed the ledger diff, full acceptance criteria, scratch
patch, retained recorder evidence and relevant ownership/cache code. No findings. The reviewer confirmed that the
ledger's claims follow from the assertions and retained output, that its limitations stay explicit, and that only the
Execution field changes. The commit/PR wording passed its fresh cold read after one rewrite.
