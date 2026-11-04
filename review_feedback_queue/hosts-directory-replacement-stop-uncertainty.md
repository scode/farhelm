# Path-based terminal shutdown has an unresolved directory-replacement premise

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A changed directory could redirect terminal shutdown, but no supported history replacing it with another installation's
matching live socket was established. Independent review does not agree to drop that remaining uncertainty; deliberate
private-directory interference is separately accepted.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations: a6e0a05fb5bf60d90d7fb1e7bfde6e787b508b3e:crates/farhelm-supervisor/src/hosts/stop.rs:358-373,468-470.

Preserve g007/f1 and g016/f1 aliases. Earlier a003/f2 agreed with full investigated unsupported-producer exclusion, but
a052/f3 actual discovery retains the incidental uncertainty and approves deliberate substitution only. Do not
majority-vote that disagreement away. a052 corrected the Rust-only remover search by finding shell rmdir in remote
uninstall, then checked stop-before-removal, sequential per-host execution and locks: this supplies an empty-directory
remover, not foreign-directory substitution. Missing premise is supported foreign matching-live-socket replacement while
shutdown is pending; unspecified external interference cannot be invented or disproved universally. Retain the actual
checks/results and exact settling evidence. This is an existing primary concern, not a new global auditor discovery.

discover (358-373) keeps the opened directory handle but constructs Target.path by joining its original pathname.
list_sockets (399-461) opens with NOFOLLOW and enumerates through that handle; stop_host (468-470) reconnects through
the pathname. The client (same commit, hosts/client.rs:1241-1264,1296-1299) uses ordinary UnixStream::connect, with no
directory identity check before accepting Welcome; shut_down (497-531) then sends Shutdown. Moving an existing hosts
directory aside and symlinking a different private hosts directory at its old name would redirect to the same socket
filename; that requires a matching name or moving the original host/socket before substitution. No such production move
was found by git grep for rename/remove_dir combined with hosts at tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (only a
provisioning test removes hosts). The later cf5188fd16a5cd4dec1e57fc257f0f08037f636d extraction still connects by
pathname.

discover retains the opened directory handle but constructs Target.path from dir.join(name) (lines 358-373);
list_sockets uses NOFOLLOW only for discovery (lines 399-414); stop_host connects through Target.path (lines 468-470),
resolving the directory again.

Replacing the hosts directory after discovery could send the requested stop to another directory's terminal and
terminate its work. The pathname resolution is confirmed; no supported producer of that directory replacement and
matching socket name is established. Original source: stop.rs:358-373,399-461,468-470,497-516. Tip source:
stop.rs:358-373,399-461,468-497,516-539 still retains a directory handle for listing/removal but resolves the original
pathname for connection and Shutdown; client.rs:639-647,1257-1315 still uses UnixStream::connect and accepts Welcome
without directory-identity verification. Synthesis checked g018/f1 and g019/f1: same-name concurrent starts are an
ordinary producer of a different host at the same pathname, but do not replace the hosts directory or cross state
directories. Those findings are not duplicates of this candidate and do not establish its missing premise. The
state-directory command intentionally ends every listed socket, irrespective of session tag (SPEC_impl.md:1804-1840).
The untagged interrupted-spawn cleanup is separate code at hosts/spawn.rs:568-603; end_host_at has no in-tree production
caller apart from stop_host at tip (git grep). g145's selective unreadable-record fallback reaches signal/shut_down at
host_launch.rs:752-780 and needs its own collision analysis, not this directory-replacement exclusion. Focused
immutable-tip git grep again found no production hosts-directory rename/remove_dir operation (only provisioning.rs:7796,
a test).

Possible coverage and caveats: these exact exclusions; the explicit symlink refusal at SPEC_impl.md:1804-1810 still
holds for a hosts symlink present when opened, so this proposal does not erase that protection. Required coverage
sources re-searched; no matching Planned item (empty), unlanded plan, BUGS entry, queue item or ledger decision found.
FILTER does not excuse an established wrong-target termination.

Additional reviewed locations: g007/f1: a6e0a05fb5bf60d90d7fb1e7bfde6e787b508b3e:SPEC_impl.md:1731-1737;
crates/farhelm-supervisor/src/hosts/stop.rs:358-370,399-415,468-479; g016/f1:
1512cdb6e7db601f8b07cfe596bddf101b1a3e45:crates/farhelm-supervisor/src/hosts/dir.rs:64-95;
c0a0c9fb126f0a6d05d72a245f19cd6288b11cbc:crates/farhelm-supervisor/src/hosts/dir.rs:99-133.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g110/f1`, `g007/f1`,
`g016/f1`. Original append-only events, revisions, withdrawals and independent judgments are retained in that run's
catalogs. Locations refer to the named reviewed revisions; the original revision context is
a6e0a05fb5bf60d90d7fb1e7bfde6e787b508b3e; persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a
source-only review; runtime occurrence and frequency were not measured.
