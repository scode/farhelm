# Uninstall creates an unreported setup lock file

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After `farhelm uninstall` (or `farhelm helm setup --uninstall` on a machine never set up), a stray hidden
`.farhelm-setup.lock`, and possibly a newly created `~/.config/systemd/user`, remains and is not mentioned.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F5 / COR-UNINSTALL-CREATES-SETUP-LOCK`, tagged **definite**. Anchors and title: `crates/farhelm/src/setup.rs:360-369`,
`crates/farhelm/src/setup.rs:745-749`, `crates/farhelm/src/setup.rs:916-924` — Uninstall and setup --uninstall create a
.farhelm-setup.lock (and possibly the unit directory) and never report it

Setup serialises its unit-file writes with a lock file, `.farhelm-setup.lock`, in the systemd user unit directory
(normally `~/.config/systemd/user`). `lock_unit_directory` creates that directory if needed (`create_dir_all`) and opens
the lock file with `create(true)` (setup.rs:916–924). The file is deliberately never deleted (comment at
setup.rs:908–911). Both uninstall paths take this lock even when they have nothing to remove:

- Standalone `farhelm uninstall` on Linux (via `remove_selected_services`, setup.rs:360–369) skips the lock only when
  nothing is selected _and_ the unit directory does not exist. On a machine where the user has other systemd user units
  but never ran `farhelm helm setup`, the directory exists, so uninstall creates `.farhelm-setup.lock` there and also
  runs a `daemon-reload` it did not need.
- The older `farhelm helm setup --uninstall` takes the lock unconditionally in a real run (setup.rs:745–749). On a
  machine with no unit directory it creates `~/.config/systemd/user` and the lock file, then reports both units
  "absent".

A lock file left by earlier setup runs also survives uninstall. Neither the uninstall preview nor its final report
mentions the file or the created directory. The existing `directory_absent` guard shows the code already tries to avoid
creating new state in the empty case; the module also says both uninstall paths share `remove_service_files` so their
behaviour "cannot drift", yet they take the lock under different conditions.

Suggested change: skip the lock (and the reload) when nothing is selected or the unit directory is absent, and after a
removal that leaves no setup-owned units, either delete the lock file while holding the lock or report it as retained.

Restater note: the finding says SPEC.md requires uninstall to leave shared directories alone. The SPEC text is
"Uninstall does not remove shared parent directories or change their permissions"; it does not forbid creating new
files, so creating an empty hidden lock file is not a direct SPEC violation. The stronger basis is that uninstall leaves
a new, unreported Farhelm file behind. Impact is small: an empty dot-file systemd ignores, and possibly an empty
`~/.config/systemd/user`.
