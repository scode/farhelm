# Codex

Farhelm tracks the foreground Codex conversation and offers Resume when its exact transcript verifies the reported
identity. A launch credential alone does not authorize a child process to replace that identity. Codex capture requires
both an attributable reporting process and matching root-conversation metadata in the reported transcript.

## Workspace trust

Codex may ask whether to trust the working directory at startup. Farhelm leaves that prompt to you. Codex 0.155.1
accepts a per-run `projects` config override for an exact absolute directory, but Farhelm compiles its launch argv
before a fresh checkout has its final accepted path. The existing hook-trust bypass is separate and does not establish
directory trust. Farhelm does not write Codex's persistent trust state.

## Launchers and wrappers

The process chain from Farhelm's reporting hook back to the owned terminal pane must contain exactly one native
executable named `codex`. The hook must use Farhelm's installed hook command. Apart from the hook, that Codex process,
and the pane's root process, every surviving process in the chain must be a shell directly invoking the hook as one
simple command. Interactive shells, shell scripts, Node launchers and other unexplained intermediaries are refused. This
restriction applies above Codex as well as between Codex and its hook.

For example, these chains can pass the process check; the transcript must still pass its separate check:

```text
pane: Codex
└── Farhelm hook

pane: launcher
└── Codex
    └── sh -c '<farhelm> internal hook …'
        └── Farhelm hook
```

This chain is refused because the Node launcher is an additional surviving intermediary:

```text
pane: shell script
└── Node launcher
    └── Codex
        └── Farhelm hook
```

Setting a wrapper's integration kind to Codex enables hook injection but does not bypass these checks. Prefer launchers
that forward the injected arguments and replace themselves with the next program using `exec`; replaced processes do not
add ancestry links. A package-manager installation is not automatically supported or refused: the surviving process
chain decides. Renaming the native Codex executable also prevents attribution. See
[agent wrappers](../agent-wrappers.md) for argument forwarding and [hook injection](../agent-hook-injection.md) for
invocation forms that skip injection.

A rejected report leaves the saved conversation and Resume offer unchanged. On a new session with no accepted report,
Resume remains unavailable. Farhelm does not scan for a different Codex conversation to compensate for an unsupported
launcher.

## Identity changes and resume

A legitimate foreground `/clear` can withdraw the old resume target before its new transcript exists. Only that exact
new transcript can make the replacement resumable. Nested native Codex processes and reports from delegated shells are
refused; root transcript metadata also separates conversations that share a process.

Historical valid `codex:` locators remain subject to exact-record verification. Historical bare IDs are retained but
cannot be resumed automatically. Missing or changed evidence refuses Resume instead of silently starting fresh.

These checks prevent accidental reporting through inherited credentials. They are not a security boundary against
another process controlled by the same Unix user that deliberately imitates the permitted process and record shapes.
