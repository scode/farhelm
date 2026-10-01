# A retried GitHub checkout create can fail permanently after a remount

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If creating a session on a fresh GitHub checkout is interrupted and retried after a reboot or remount (on btrfs, NFS,
overlayfs and similar), the clone can be refused as "the directory was replaced" and the create fails permanently, on
exactly the filesystems #1200 was meant to handle.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F33 / COR-CHECKOUT-DEV-INO`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/launch.rs:1199` — a retried fresh-checkout create still checks the folder's raw device
number, which #1200 stopped trusting elsewhere.

When a user creates a session on a fresh GitHub checkout, the supervisor first creates an empty folder and records its
filesystem identity (device number and inode). The actual `git clone` is done later by the launch shim, the small part
of the farhelm binary that runs inside the session's terminal before the agent starts. The shim re-checks that the
folder is still the one that was allocated by comparing the raw (device, inode) pair (`launch.rs:1197-1229`) and refuses
to clone into anything else.

PR #1200 (commit 1774435) taught the supervisor's working-copy registry that device numbers are not stable. Btrfs, NFS,
overlayfs and device-mapper setups renumber devices on remount or reboot. So the registry now accepts a folder whose
device changed as long as its inode and birth time match (`same_directory`,
`crates/farhelm-supervisor/src/working_copies.rs:548-555`). The create-retry path uses that rule: when a create was
interrupted after the folder was allocated, the retry calls `verify_identity` and accepts the folder on `Matches`
(`core.rs:8904-8916`). But it then hands the shim the _stored_ old (device, inode) (`core.rs:9040-9041`), and the shim
still compares them raw.

So a create interrupted after the folder was made, then retried after a reboot or remount that changed the device
number, is refused by the shim with "the directory … was replaced since it was allocated". The shim records the
preparation as Failed at stage `cwd-identity`. It does this with overwrite enabled, before reading the existing
preparation state. The retry path explicitly allows reuse of an intact Ready record (`publish_preparation_not_started`),
so this can overwrite a correct Ready record left by a clone that had already finished. Setup is never retried. The
trigger is narrow, but a false "folder replaced" refusal permanently fails the create on exactly the filesystems #1200
was meant to support. Suggested change: carry the folder's birth time in the preparation handed to the shim and apply
`same_directory`'s rule there, or have the retry path pass the folder's _current_ (device, inode) once `verify_identity`
has returned Matches.
