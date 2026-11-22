# Restart unwind comment claims a failed sweep preserves a host record already removed

Reviewed commit: cfcc29bf0794c06ac876d2fe665bdbefe7d602f2

## TLDR

The Restart cleanup comment promises a host record survives a failed sweep even though removal happens first.

## Details

**other.** Source inspection.

SPEC_impl.md:2087–2100 and crates/farhelm-supervisor/src/service/core/host_restart.rs:726–732,745–777,
cfcc29bf0794c06ac876d2fe665bdbefe7d602f2; SPEC_impl.md:2089–2102 and host_restart.rs:731–737,750–782,
1a53d9d35889d7dde96e31320cc1681dd013737d

Scope recheck: assigned patch cfcc29bf0794c06ac876d2fe665bdbefe7d602f2 replaces the original terminal-end-only exception
with distinct terminal-end and process-tree outcomes and explicitly adds that a sweep failure follows attempted
host-record removal. It changes SPEC_impl.md and leaves the older function comment intact. TLDR: The comment can
misdirect recovery diagnosis. It says either kind of failed cleanup retains both records, but an unconfirmed host end
returns before unlink, while an otherwise-confirmed host end leads to attempted unlink before the process sweep. A
failed sweep can therefore keep the database launching state without keeping the failed replacement's host record, as
the amended spec says. The reviewer suggested other, confidence confirmed. Proposed_drop no; identified.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `implementation-contracts/IC-09`.
