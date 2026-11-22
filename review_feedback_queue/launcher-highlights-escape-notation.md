# Launcher search can underline escape notation that did not match the query

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

The launcher can underline the escape notation displayed for a path even when that notation was absent from the matching
path.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/host_text.rs:385–394,425–441,515–524,555–565;
crates/gui/farhelm-gui/src/launcher/search.rs:550–551,688–698;
crates/gui/farhelm-gui/src/launcher/view/search.rs:931–936

The saved-folder filter matches each raw path by substring; its comment explicitly says an escape's spelling must not
stand for the underlying character. Once a row is admitted, HostSentence searches the fully escaped rendering for every
occurrence. A path with an actual matching substring plus a hidden character whose escape contains that substring gets
an extra underline inside the escape. The row remains correctly escaped and its action uses the raw folder. This
misstates where the match is, with no established redirection or security impact. Project the raw matched character
ranges through escaping instead of matching again in the display string. The implementation in host_text.rs is unchanged
at pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-hosts/launcher-hosts-escape-highlight`.
