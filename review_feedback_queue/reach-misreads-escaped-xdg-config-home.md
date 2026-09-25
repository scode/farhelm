# The reach check misreads an escaped XDG_CONFIG_HOME

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A host whose config directory path contains a space is refused automatic setup with a misleading "relative
XDG_CONFIG_HOME" message.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F24 / COR-XDG-CONFIG-ESCAPED`, tagged **possible**. Anchors and title: `provisioning/backend.rs:1678-1683`,
`provisioning/backend.rs:2114-2122` — The reach check misreads a shell-escaped XDG_CONFIG_HOME

To find where systemd looks for user units, the reach script reads `XDG_CONFIG_HOME` from
`systemctl --user show-environment` with `sed -n 's/^XDG_CONFIG_HOME=//p'` and accepts it only if it starts with `/`
(backend.rs:1678-1683). According to the reviewer, recent systemd prints values that need quoting in shell-escaped form,
for example `XDG_CONFIG_HOME=$'/home/u/my config'`. The extracted text then starts with `$`, fails the "starts with /"
test, and the script reports `unsupported-xdg`, which `parse_reach_output` (backend.rs:2114-2122) turns into "the
systemd user manager reports a relative XDG_CONFIG_HOME". The host is sent to manual setup for a false reason, although
its unit directory is absolute and usable.

Suggested change: undo systemd's escaping before classifying (the value comes from the user's own manager), or refuse
with a message that shows the escaped value instead of calling it relative.

Restater note: I did not verify which systemd versions escape `show-environment` output this way; the parsing weakness
is confirmed from the script, the trigger rests on the reviewer's account of systemd's output format.

User-visible consequence: a host whose config directory path contains a space is refused automatic setup with a
misleading "relative XDG_CONFIG_HOME" message.
