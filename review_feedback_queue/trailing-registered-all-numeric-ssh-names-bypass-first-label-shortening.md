# Registered all-numeric SSH names can bypass first-label shortening

Reviewed commit: 39b696fc3fa5904c0b6b170eb08b2414de0cfee4

## TLDR

A multi-label numeric SSH name with too many components to be IPv4 can be saved in a host's Destination field. The
collapsed rail then uses the whole spelling in its short caption and can truncate it. Hover help still has the full
name.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/host_name.rs:47`.

SPEC.md:288–295 requires the first label for SSH hostnames and preserves IP addresses, including shorthand.
host_name.rs:44–56 classifies every string of ASCII digits and dots as an address, including names with more than four
numeric components. The parent at :36–48 classified actual IPs separately and took the first label otherwise. Farhelm
input acceptance is source-established. The desktop Destination editor posts its draft verbatim at
crates/gui/farhelm-gui/src/hosts/settings.rs:265–283 and :667–695. The helm retarget route at hosts.rs:869–883 and
:899–912 uses the shape gate at store.rs:2398–2403 through :7848–7887, then writes the spelling before reconciliation
(:7946–7958). The gate accepts nonempty dotted numeric input. The registration route at hosts.rs:759–804 likewise saves
through store.rs:7173–7187. Its existing assertion at hosts.rs:2038–2060 confirms registration of an unreachable host
and the returned destination. crates/gui/farhelm-gui/src/hosts/identity.rs:53–67 passes the ordinary unaliased
destination to the helper, and session_list/host_rail.rs:727–747 draws the truncated caption. The separate Add host
dialog offers SSH config names at hosts/add.rs:1554–1567 but probes first. A successful OpenSSH match or connection for
a specific numeric alias is not established, and neither is required for the direct Destination edit or registration.
host_name.rs:124–169 does not cover this non-address input class. Limit the preservation rule to the address spellings
it intends to cover. Bucket: other, display only under SPEC.md:293–295. No connection or identity-resolution effect
established. Confidence: confirmed by pinned source for the accepted input, caller, and label derivation. No runtime
observation. Possible coverage: abbreviated-ipv4-short-label-scope-ambiguity.md, named in the reviewed commit.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `status/f1`.
