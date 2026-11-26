#!/usr/bin/env python3
"""Git credential helper for the agent-records repo in Cloudflare Artifacts: answers `get` from a repo token file.

Artifacts accepts a repo token over git's HTTPS transport in two forms. The records tool (`cargo xtask records`) sends
the whole token string as `Authorization: Bearer <token>` through its git children's environment. Plain git, which the
attended flows (triage, releases, deflake, run-all-tests) use on their own working clones, can only send what a
credential helper gives it, as HTTP Basic auth; for that form Cloudflare's rule is any username and, as the password,
the token's part before its `?expires=<unix seconds>` suffix. This helper applies that rule, so the token stays in its
file (mode 600) and never lands in a remote URL, a clone's config, a command line or the shell history.

An attended clone is configured once, when it is made, with two `credential.helper` entries: an empty one, which drops
any helper inherited from the user's or system's git config (a keychain helper would otherwise store the token), and
then this script with the token file as its one argument. Both paths must be absolute (the script is named by its
own path, which its shebang runs, not through an interpreter prefix), because git runs the entry through the shell as
`<script> <token file> get`:

    git clone -c credential.helper= \\
      -c 'credential.helper=<repo-checkout>/scripts/records-git-credential.py <home>/.fh-records/agent-records_token' \\
      <remote> <dir>

Each clone points at the helper in the checkout that owns the clone (one checkout per clone), so the helper's version
travels with that checkout.

Behavior: `get` reads git's attribute lines from stdin and, when the protocol is `https` and the host is an Artifacts
git host (`*.artifacts.cloudflare.net`), prints `username=x` and `password=<token before ?expires=>`. For anything else
it prints nothing, so the token is never offered over plain HTTP or to another server (a redirect, a submodule, or a
helper entry that ended up in a broader config than the one clone). `store` and `erase` read and ignore their input:
the token file is the only store, and git must not delete it after a failed attempt. A missing or empty token file
exits 1 with a message on stderr; git then reports the authentication failure. Wrong usage exits 2.
"""
import sys

# Cloudflare's tokens carry their expiry after this marker; the Basic-auth password is what comes before it.
EXPIRES_MARKER = "?expires="
# Artifacts serves git at https://<account id>.artifacts.cloudflare.net/git/...; the token is for no other host.
ARTIFACTS_HOST_SUFFIX = ".artifacts.cloudflare.net"


def read_attributes(stream):
    """Git's request: `key=value` lines up to a blank line or EOF. Repeated keys keep their last value."""
    attributes = {}
    for line in stream:
        line = line.rstrip("\n")
        if not line:
            break
        key, _, value = line.partition("=")
        attributes[key] = value
    return attributes


def password_from_token(token):
    """The Basic-auth password for a repo token: the part before `?expires=`, or the whole token without one."""
    return token.split(EXPIRES_MARKER, 1)[0]


def main(argv, stdin, stdout, stderr):
    """Runs one helper action; returns the exit status. Streams are parameters so tests could drive it in-process."""
    if len(argv) != 2 or argv[1] not in {"get", "store", "erase"}:
        print("usage: records-git-credential.py <token file> get|store|erase", file=stderr)
        return 2
    token_file, action = argv
    attributes = read_attributes(stdin)
    if action != "get":
        return 0
    host = attributes.get("host", "").lower()
    # git sends `host` with any `:port`; Artifacts uses the default port, so a port is just another foreign host.
    if attributes.get("protocol") != "https" or not host.endswith(ARTIFACTS_HOST_SUFFIX):
        return 0
    try:
        with open(token_file, encoding="utf-8") as handle:
            token = handle.read().strip()
    except OSError as error:
        print(f"records-git-credential: cannot read the token file {token_file}: {error.strerror}", file=stderr)
        return 1
    password = password_from_token(token)
    if not password or "\n" in password:
        print(f"records-git-credential: the token file {token_file} must hold one token", file=stderr)
        return 1
    stdout.write(f"username=x\npassword={password}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:], sys.stdin, sys.stdout, sys.stderr))
