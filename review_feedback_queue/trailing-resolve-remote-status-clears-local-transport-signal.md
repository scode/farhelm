# resolve_remote_status clears local transport signal termination and can return truncated stdout as exit 0

Reviewed commit: 72dcafbc581259b7411c16f24995dab9f25f0986

## TLDR

If the local SSH or Podman client process is killed by a signal just after the remote command prints its status record
on stderr, Farhelm can discard the signal termination and treat the command as having succeeded, even if its stdout was
cut off mid-stream.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/backend.rs:1718-1737`.

In `crates/farhelm-helm/src/provisioning/backend.rs:1718-1737`, `CommandResult::resolve_remote_status` only preserves
`self.code == Some(255) && route_uses_ssh(route)` as a transport failure. If the local `ssh` or `podman` process was
terminated by a signal (`self.code == None`, `self.signal == Some(sig)`), and `stderr` had already received the
wrapper's status record (e.g. `0`), lines 1735-1736 execute `self.code = Some(code); self.signal = None; Ok(self)`,
erasing the transport signal death. Because SSH carries stdout and stderr on separate channel streams and local pipes, a
signalled local transport can deliver EOF on stdout with truncated bytes after stderr has delivered the record. Callers
whose stdout lacks an explicit framing terminator — such as `host_home` (`backend.rs:2489-2503`, `printf '%s' "$HOME"`,
which only checks `home.is_absolute()`), `prepare_sandbox` (`tensorlake_backend.rs:118-128`), `inspect`
(`backend.rs:2874`, `uname -s`), and `remove_unit`/`stop`/`remove_file`/`remove_directory` (`stdout == b"absent"`) —
would then act on truncated stdout as an exit-0 success. A lossy SSH server exits normally with
`code: Some(0), signal: None`, whereas a remote command killed by a signal is caught by the wrapper shell and exits with
`128 + sig` (or SSH exit 255 if the wrapper itself died), so `self.code.is_none()` / `self.signal.is_some()` always
indicates abnormal termination of the local transport process itself. The same pattern appears in `probe`
(`backend.rs:2809-2821`), where `!(code == Some(255) && route_uses_ssh(route))` also overrides `code` when
`status.code()` is `None`. Fix direction: in `resolve_remote_status`, return `Ok(self)` (preserving
`code: None, signal: Some(sig)`) whenever `self.code.is_none()` (or `self.signal.is_some()`), matching the SSH exit-255
transport-failure handling; likewise in `probe`, do not overwrite `code` with `stderr.command_status` when
`status.code().is_none()`.

Reviewer metadata (g33-remote-supervisor-exit-status/f2): suggested bucket: other (could NOT be in highest; requires an
external signal killing the isolated local transport child in the narrow window after stderr delivers the status record
and before stdout drains, and callers either reject malformed output or fail safely on a truncated path); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g33-remote-supervisor-exit-status/f2`.
