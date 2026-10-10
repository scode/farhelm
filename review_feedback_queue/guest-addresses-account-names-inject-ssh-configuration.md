# Guest addresses or account names inject SSH configuration or shell commands

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An IPv6 scope suffix could inject text into guest SSH configuration.

## Details

F320 — **possible** — `releasing/mac-vm-test/network.py:28–36` — Guest addresses or account names inject SSH
configuration or shell commands

Address normalization preserves IPv6 scope text under the inspected interpreter, including whitespace or newlines that
are later interpolated into HostName. Username and shell-argument protections close different routes. Whether the
guest-VM tool Tart can emit dangerous text, and target-interpreter applicability, remain unverified. Validate the
normalized address against SSH configuration's token and line grammar before interpolation, rejecting unsafe scope
characters.

## Evidence and triage context

- network.py:28 converts ipaddress.ip_address(address) back to text and :36 interpolates it into HostName. The caller at
  :92 obtains stripped Tart ip output; route_via_gateway is optional at :93–94. The inspected Python implementation,
  /usr/lib/python3.12/ipaddress.py:1898–1903 and :1953–1962, preserves scope text and rejects an empty scope, additional
  percent signs and slashes, but does not reject embedded whitespace/newlines. An address-shaped string with injected
  scope text can therefore survive this guard. Username fullmatch at network.py:29 and JSON stdin at :125–127 close
  their respective routes, not this one. Dangerous Tart output and applicability to the target interpreter were not
  demonstrated; no exact coverage basis was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_07_sec:p1:C2`.

- `automation_website_07_sec:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
