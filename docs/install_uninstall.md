# Installing and uninstalling Farhelm

This describes the standalone shell installer and `farhelm uninstall`. It covers the installation on your machine;
removing software from hosts provisioned by a helm is a separate operation.

## Installation and updates

The installer supports Linux on x86-64 or ARM64 and macOS on Apple silicon, including when invoked under Rosetta. Intel
Macs and other platforms are unsupported. Installation needs `curl`, `tar` with gzip support, and one of `sha256sum`,
`shasum` or `openssl` for checksum verification. Machines that run sessions also need tmux 3.7c or newer, installed
separately; missing tmux does not prevent installation itself.

Run the installer as your normal user:

```sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | sh
```

It downloads release archives for your platform and checks their checksums before installing the executables. The
default location is `~/.local/bin/farhelm`. On macOS, it also installs `~/.local/bin/farhelm-desktop` and creates
`~/Applications/Farhelm.app`, with copies of the executables inside the bundle.

Set `FARHELM_INSTALL_DIR` to choose another executable directory. The macOS app still goes in `~/Applications`;
`FARHELM_NO_APP_BUNDLE=1` skips creating or updating that bundle. Set installer options on the `sh` side of the pipe,
for example:

```sh
curl -fsSL https://raw.githubusercontent.com/scode/farhelm/main/scripts/install.sh | FARHELM_INSTALL_DIR="$HOME/bin" sh
```

Re-run the installer to update, using the same custom directory if you chose one. It defaults to the latest stable
release; `FARHELM_VERSION` selects a specific version, including a prerelease. There is no automatic updater. Updates
preserve user data, but already-running processes need restarting to use the new executables. Quit the desktop app
before updating and relaunch it afterward. Follow the installer's restart guidance for services and other processes.

Older releases without app-bundle resources install the executables but leave any existing app bundle unchanged. After
selecting such a release, launching that existing app can therefore still run its previous version. The app-bundle
opt-out also leaves an existing bundle unchanged.

Installation does not start Farhelm or register services. On Linux, `farhelm helm setup` is a separate operation that
installs and starts systemd user services for a machine intended to run a helm and supervisor. Desktop users do not need
that setup: the desktop app manages its own helm and local supervisor.

## Installation records

The installer writes hidden files that record which installation it created and the contents of its installed files.
Uninstall uses these records to recognize its own artifacts before deleting them. They record no running processes or
sessions.

| Record file                                                 | Contents, in field order                                                                                                                                                                                                                           |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `~/.local/bin/.farhelm-installation`                        | The identifier `farhelm-standalone`; the absolute executable-directory path with symlinks resolved; the SHA-256 checksum of `farhelm`; the checksum of `farhelm-desktop`, or an empty field on Linux.                                              |
| `~/Applications/Farhelm.app/Contents/.farhelm-installation` | The identifier `farhelm-app`; the absolute executable-directory path this bundle came from; checksums of the bundle's own `Contents/MacOS/farhelm`, `Contents/MacOS/farhelm-desktop`, `Contents/Info.plist` and `Contents/Resources/Farhelm.icns`. |
| `~/Applications/.Farhelm.app.uninstall-receipt`             | A temporary copy of the bundle record, created during uninstall so an interrupted removal can be retried after the internal record has gone.                                                                                                       |

With a custom `FARHELM_INSTALL_DIR`, the first record lives in that directory, beside `farhelm`. The app paths remain
the same. The records contain NUL-separated fields, including a final NUL; checksums are lowercase SHA-256 hex strings.
They are not line-oriented text files. The installer creates its records with owner-only read/write permissions
(`0600`). Uninstall requires records to belong to the invoking user and refuses records writable by other users.

The executable-directory record is refreshed when the installer replaces the executables; the bundle record describes
the copies inside that bundle. If an update leaves an older bundle untouched, its record continues to describe those
older files. A checksum mismatch means the current contents differ from the recorded installation. These records do not
grant permission to delete arbitrary paths: removal is restricted to the known Farhelm filenames and bundle layout.
Leave the records in place while the installation exists or removal needs retrying.

## Uninstalling

Uninstall first verifies that the files it would remove belong to the selected installation. It compares them with the
installation records above and checks their contents, file types and filesystem ownership. These are the **ownership
checks** described below; they concern files, not running processes.

Before removing the software, stop local sessions and their additional terminals, quit the desktop app, and stop
manually started Farhelm processes. **This is your responsibility: uninstall does not detect live sessions or refuse
because they are running.** Sessions can survive their supervisor, so quitting the app or supervisor alone is not
enough. Stop any custom services yourself. Farhelm automatically stops its recognized setup-owned Linux services during
uninstall.

Preview the operation without changing files or stopping services:

```sh
farhelm uninstall --dry-run
```

Then run `farhelm uninstall` and confirm once. For noninteractive use, `farhelm uninstall --yes` skips the prompt; the
same ownership checks still apply. If your release has no uninstall command, update with the installer once to get it.

The command selects the installation containing the CLI you invoked. With multiple installations, invoke the intended
executable by its full path and check the preview. A symlink used to invoke the CLI resolves to its installation. Use
the CLI in the executable directory, rather than its copy inside `Farhelm.app`; an app-local invocation directs you to
the appropriate CLI.

On macOS, uninstall refuses if `~/Applications/Farhelm.app` belongs to a different executable directory. Choosing a CLI
by full path does not bypass that check: all standalone installations share this one app-bundle location.

## What protects against inappropriate deletion

The ownership checks happen before any files or services are removed. A familiar filename is not enough to authorize
deletion: changed contents, invalid ownership records, or unexpected entries inside the app bundle cause a refusal.
Uninstall also checks file types and ownership; a symlink in place of a claimed file cannot redirect deletion to its
target. It removes only recognized files and the app's empty directories, rather than recursively deleting whatever
happens to be inside an installation directory.

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

These protections assume you keep installation, updates, setup and Farhelm startup stopped until uninstall finishes. The
command does not inspect running processes or prove that sessions have stopped, and it does not protect against
concurrent changes to the installation.

## Refusals and interrupted removal

A refusal names the path and the check or concrete error that prevented removal. Investigate that evidence rather than
deleting the named file just to get past the check. Missing or invalid ownership records may require rerunning the
installer to repair the installation. Preserve any intentional modifications before reinstalling.

Removal can fail after some work has completed, for example if stopping a service or deleting a file fails. The output
reports completed actions and the failure. The CLI is removed last, after the other required removal steps, so you can
resolve the problem and rerun the same command. Already-removed files do not by themselves prevent retrying.

An interrupted macOS removal may leave `~/Applications/.Farhelm.app.uninstall-receipt` so the next attempt can recognize
the remaining directories safely. Leave that file in place for the retry; successful bundle removal clears it. If only
tidying the final executable-directory ownership record fails after the CLI is gone, uninstall reports the leftover
record without treating the software removal as failed. Successful uninstall retains user data and does not mean every
trace of Farhelm has been erased.
