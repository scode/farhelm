# Installing and uninstalling Farhelm

This describes the standalone shell installer and `farhelm uninstall`. It covers the installation on your machine;
removing Farhelm from a host the helm set up over SSH is a separate operation, the uninstall item in that host's menu in
the hosts panel (see the docs site's Manage hosts page).

## Installation and updates

The installer temporarily supports only the macOS desktop app on Apple silicon, including when invoked under Rosetta.
Intel Macs and other platforms are refused. Only the installer is limited: Linux remains supported for helms and session
hosts, and a helm sets up its remote hosts over SSH.

Installation needs `curl`, `tar` with gzip support, and one of `sha256sum`, `shasum` or `openssl` for checksum
verification. The Mac also needs tmux 3.7c or newer to run sessions, installed separately; missing tmux does not prevent
installation itself.

Run the installer as your normal user:

```sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | sh
```

The installer checks the release archives' checksums and installs `~/Applications/Farhelm.app`. The app is the whole
installation and the way to start Farhelm: open it from Spotlight, Launchpad or `~/Applications`. For the Terminal, the
installer also makes `~/.local/bin/farhelm` a link into the app, so `farhelm` works in a shell whose `PATH` includes
`~/.local/bin`. Those locations are fixed. Keep both directories writable only by you: the update safeguards assume
another account cannot create or replace files there.

Re-run the installer to update. It defaults to the latest stable release; `FARHELM_VERSION` selects a specific version,
including a prerelease. Set it on the `sh` side of the pipe:

```sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | FARHELM_VERSION=0.2.1 sh
```

There is no automatic updater yet. Updates preserve user data.

You can update while Farhelm is open. The running Farhelm keeps working as it was, on the version it started with:
sessions keep running, new sessions start, and agents' `farhelm` commands and conversation tracking keep working. Quit
and reopen Farhelm to finish the update; after that, everything, including sessions started before the update, uses the
new version. Reopening right after quitting is fine: the new Farhelm waits a few seconds for the old one to finish
shutting down.

How that works, in case you look inside the app: each version's `farhelm` lives in its own folder,
`Farhelm.app/Contents/Versions/<version>/`, and `Contents/Versions/installed` names the one the next start uses. An
update adds the new version's folder, then replaces the app's main program, then `installed`, then `Info.plist`, so
stopping it at any point leaves an app that starts either the old version or the new one; re-running the installer
finishes the job. `Contents/MacOS/farhelm` is a small script that runs the right version: the one the running Farhelm
started from, for commands that come from inside its sessions, and otherwise the installed one. Each update removes
version folders other than the new one, the one it replaced, and the one a running Farhelm started from.

The first update from a release that installed copies of `farhelm` and `farhelm-desktop` in `~/.local/bin` converts the
installation to this layout: quit Farhelm before that update. The installer removes those copies when their checksums
show they are the ones it put there (the record below), and rebuilds the app. The installer only replaces an app it can
tell it built: one carrying its current record, or, for this conversion, the previous layout's record naming
`~/.local/bin`, or the recordless app the installers of early September 2026 built. Any other `Farhelm.app` (one you
built or customised, or one recorded for another installation directory) is left untouched and the installer exits with
an error before changing anything; rename or remove it and re-run the installer.

A `farhelm` in `~/.local/bin` that the installer did not put there, such as a wrapper script of your own, is kept under
a visible name like `farhelm.replaced-20260928T221500Z` in the same directory, and the installer's closing message names
it. Delete the kept copy once you no longer need it. A `farhelm-desktop` of your own in `~/.local/bin` is left alone.

Only one installer run changes `Farhelm.app` at a time. A run that finds `~/Applications/.farhelm-app.lock`, held by
another run or left behind by an interrupted one, exits with an error before changing anything. Re-run the installer; if
the error persists and no installer is running, remove that lock by hand.

Releases that cannot be updated while running (from before this layout) and releases without the Mac app resources are
refused before anything changes. Leave `FARHELM_VERSION` unset for the latest stable release.

Installation does not start Farhelm or register services. The desktop app manages its own helm and local supervisor.

## Installation records

The installer writes a hidden record inside the app that names the installation it created. Uninstall uses it to
recognize its own artifacts before deleting them. It records no running processes or sessions.

| Record file                                                 | Contents, in field order                                                                                                                                       |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `~/Applications/Farhelm.app/Contents/.farhelm-installation` | The identifier `farhelm-app-v2`; the absolute path of the Terminal link this installation owns (`~/.local/bin/farhelm`, with symlinks in the folder resolved). |

The record is NUL-separated, with a final NUL; it is not a line-oriented text file. The installer creates it with
owner-only read/write permissions (`0600`). Uninstall requires it to belong to the invoking user and refuses a record
writable by other users. It holds no checksums, because every other file in the app changes on every update; uninstall
checks the app's layout and file ownership instead (below). Leave the record in place while the installation exists or
removal needs retrying.

Running Farhelm also keeps a one-line `running-version` file in its state directory (`~/.local/state/farhelm` unless
`XDG_STATE_HOME` says otherwise), naming the version it started from. The installer reads it to keep that version, and
uninstall removes it.

Older Linux installations, made by installers from before the installer became Mac-only, keep their record
`~/.local/bin/.farhelm-installation` (or beside `farhelm` in a custom directory): the identifier `farhelm-standalone`,
the absolute executable-directory path with symlinks resolved, and the SHA-256 checksums of `farhelm` and (empty)
`farhelm-desktop`. Uninstall still uses it there.

## Uninstalling

Uninstall first verifies that the files it would remove belong to the selected installation. On a Mac it reads the app's
record above and checks that the app holds exactly the files and folders the installer creates, as real files and
folders owned by you; on Linux it compares the installed files with the record's checksums. These are the **ownership
checks** described below; they concern files, not running processes.

Before removing the software, stop local sessions and their additional terminals, quit the desktop app, and stop
manually started Farhelm processes. **This is your responsibility: uninstall does not detect live sessions or refuse
because they are running.** On macOS it does refuse while the desktop app (or another Farhelm supervisor or helm using
the default state directory) holds its state-directory lock. Sessions can survive their supervisor, so quitting the app
or supervisor alone is not enough. Stop any custom services yourself. Farhelm automatically stops its recognized
setup-owned Linux services during uninstall.

Preview the operation without changing files or stopping services:

```sh
farhelm uninstall --dry-run
```

Then run `farhelm uninstall` and confirm once. For noninteractive use, `farhelm uninstall --yes` skips the prompt; the
same ownership checks still apply. If your macOS release has no uninstall command, update with the installer once to get
it. The installer cannot provide that upgrade on Linux.

The command selects the installation containing the CLI you invoked. On a Mac, run it as
`~/.local/bin/farhelm uninstall` (or `farhelm uninstall` when `~/.local/bin` is on your `PATH`), which reaches the
installed version inside the app; a `farhelm` that is not inside an installed app refuses and names that command. It
removes the whole app, every version kept in it, the `running-version` file, and the Terminal link, if the link still
points into the app; a `~/.local/bin/farhelm` that is something else by then is left in place and reported. On Linux,
invoke the intended installation's CLI by its full path when there are several, and check the preview; a symlink used to
invoke the CLI resolves to its installation.

## What protects against inappropriate deletion

The ownership checks happen before any files or services are removed. A familiar filename is not enough to authorize
deletion: changed contents, invalid ownership records, or unexpected entries inside the app bundle cause a refusal.
Uninstall also checks file types and ownership; a symlink in place of a claimed file cannot redirect deletion to its
target. It removes only recognized files and the app's empty directories, rather than recursively deleting whatever
happens to be inside an installation directory or the app.

On Linux, a service is selected only when its systemd unit file identifies it as managed by Farhelm setup and names an
executable belonging to this installation. These are `farhelm-helm.service` and `farhelm-supervisor.service` in the user
systemd user unit directory, normally `~/.config/systemd/user` (or beneath `XDG_CONFIG_HOME`). Their first line must be
`# managed-by: farhelm helm setup`, and their `ExecStart` executable must resolve to this installation's CLI. Selected
services are stopped and disabled before their files or executables are removed. Custom services and systemd unit files
belonging to another installation are retained. Systemd service drop-ins are retained and reported, and uninstall leaves
user lingering settings alone. It does not interpret overrides to establish what a service is actually running; review
and stop custom service arrangements yourself.

Your session history, attachments, credentials, host registry, preferences, logs and cached payloads remain. The output
names the known retained state location; custom data locations also remain untouched. Project directories, agent tools
and their data, separately installed dependencies such as tmux, unrelated files, and shared directories such as
`~/.local/bin` and `~/Applications` are preserved. There is no purge option. Uninstalling a helm does not remove remote
installations or stop sessions on remote hosts.

Uninstall also refuses, with nothing removed, when an install, update or setup of this installation holds its lock, and
when what it is about to remove changed after you confirmed; an install or update started while uninstall is running
refuses in turn, and so does a setup when Farhelm's services are set up. If you interrupt an uninstall after confirming
(Ctrl-C, a closed terminal), it can leave `~/Applications/.farhelm-app.lock` on a Mac, or `.farhelm-install.lock` in a
Linux installation's directory, and the retry then refuses and names it: once nothing is running, remove it as the
message says. The command does not inspect running processes or prove that sessions have stopped.

## Refusals and interrupted removal

A refusal names the path and the check or concrete error that prevented removal. Investigate that evidence rather than
deleting the named file just to get past the check. Missing or invalid ownership records may require rerunning the
installer to repair the installation. Preserve any intentional modifications before reinstalling.

Removal can fail after some work has completed, for example if stopping a service or deleting a file fails. The output
reports completed actions and the failure. Uninstall removes everything else before the files `farhelm uninstall` itself
needs (on a Mac the Terminal link, the script in `Contents/MacOS/farhelm`, `Contents/Versions/installed`, the folders of
the version it runs as and the installed version; on Linux the CLI), so you can resolve the problem and rerun the same
command. Already-removed files do not by themselves prevent retrying. If removal fails within that last group on a Mac,
what remains of `Farhelm.app` holds no data; move it to the Trash. If only tidying the final executable-directory
ownership record fails after a Linux CLI is gone, uninstall reports the leftover record without treating the software
removal as failed. Successful uninstall retains user data and does not mean every trace of Farhelm has been erased.
