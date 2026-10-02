---
kind: fixed
---

A Codex session launched with a command that already resumes or forks a conversation, such as
`codex resume <id>`, could not be resumed or restarted: Farhelm added a second `resume` and Codex refused to start.
Creating such a session now fails up front with a message saying so. To keep such a launch resumable, start it from a
profile that sets its own resume command. Sessions created before this change keep the resume command they were given
and still fail to resume; recreate them from a profile that sets its own resume command. Any argument spelled exactly `resume` or `fork`
counts, including an option value such as a Codex config profile named `fork`.
