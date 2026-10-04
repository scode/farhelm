---
kind: breaking
---

Agent profiles are gone. The Profiles button beside New, the profile picker in the session launcher, and the built-in profiles are removed, and profiles you saved are deleted at the upgrade with no conversion. To start a session, choose an agent (Claude, Codex, and so on) in the session launcher, or choose "other / command" and type the command line to run. Sessions you already created from a profile keep running and can still be restarted when they can resume; their row no longer shows the profile's name. Cloning or replacing one runs its stored command line only, so a session whose profile set its own agent type or resume command loses conversation tracking and Restart in the copy.

A typed command cannot yet say which agent it runs or how to resume it, which a profile could. Until a later change in this release adds that, a command that already picks a conversation (such as `claude --continue`) is refused, and a command started through a wrapper program gets no conversation tracking or Restart.

For agents using the `farhelm agent` commands: `farhelm agent profiles` is refused, `farhelm agent create` takes the command line to run with `--invocation` and refuses `--profile` and `--profile-id`, and `farhelm spawn` takes only `--inherit-agent` and refuses `--agent` and `--profile-id`; each refusal says what to use instead. In `farhelm agent sessions`, a session's agent is always the agent's own name (such as `claude`) or `custom`, never a profile name; the `--json` schema version is now 4. Retrying a create or spawn that named a profile, with an idempotency key used before the upgrade, is refused rather than repeated.

Downgrading to an earlier release after this one is not supported, and removed profiles are not restored.

A later change in the same release introduces launch templates and agent and command launches, which may supersede parts of this entry; curation should merge them.
