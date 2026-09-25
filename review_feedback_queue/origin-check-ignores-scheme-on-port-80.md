# Origin check ignores the scheme, accepting https on a port-80 helm

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

None unless the helm runs on port 80 and something else serves HTTPS on loopback; a hole in a defence layer, not a login
bypass.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F10 / COR-HTTPS-PORT80`, tagged **definite**. Anchors and title: `middleware.rs:100-113` — The origin check ignores the
scheme, so on `--port 80` it accepts `https://127.0.0.1` (port 443) as the helm's own origin

Inside the origin guard, `is_loopback_authority` strips either `http://` or `https://` from an `Origin` and compares
only the remaining host and port against the helm's loopback addresses. Browsers leave the port out of `Host` and
`Origin` when it is the scheme's default, so for a helm on port 80 the function also accepts the bare forms `127.0.0.1`,
`localhost`, and `[::1]` (lines 100–113). The default port depends on the scheme, though. `Origin: https://127.0.0.1`
means port 443, yet it passes for a port-80 helm, because the scheme was already thrown away.

For any other port, an `https://127.0.0.1:<port>` origin also passes. That case is harmless: the helm itself owns that
port with plain HTTP, so no TLS page can be served from it. Only `--port 80` opens a real gap, and it lets a TLS page
served on loopback port 443 pass the guard as if it were the helm's own page.

This is a gap in one layer of defence, not a way in: the request still needs a device secret. It also needs an unusual
setup. The helm has to run on port 80 and something else has to serve HTTPS on loopback 443. On Linux, binding either
port normally needs extra privilege; macOS lets ordinary users bind low ports on any address.

Suggested fix: accept only `http://` origins, since the helm never serves TLS, and apply the bare-host exception only to
`Host` and to `http://` origins.
