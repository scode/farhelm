# README demo video: intent

NOTE: THROWAWAY SAMPLE. Nothing below is the intended demo video, and none of it was chosen for what it says about
Farhelm. It exists only to show that the recording machinery works end to end (title cards, callouts, a highlight with a
spotlight, a caption, pointer clicks, answering a question and typing into a live terminal, and a cut). When the real
video is designed, this file, `scenario.json5`, the transcripts, and the beats in `e2e/readme-video/capture.spec.ts` are
replaced wholesale; do not polish or extend them.

This file is the script of the demo video, in the sense a film has a script: what happens, in what order, and what the
annotations say. It is written for a person and for the agent that turns it into `e2e/readme-video/capture.spec.ts`, and
it changes only when the maintainer wants a different video. It deliberately says nothing about selectors, coordinates,
or timings in milliseconds; those are the capture spec's business and change whenever the UI does. `SPEC.md` in this
directory says how the two relate.

Quoted text is the exact wording of an on-screen annotation. Everything else describes intent, and the capture spec may
realise it however the current UI allows.

## The fleet

The fleet is `scenario.json5`: five sessions across three hosts. One is working and open when the video starts, one is
waiting on a question, and the rest fill out the list with an unread idle session, a second working one, and a finished
one.

## Beats

1. Title card: "farhelm", with the subtitle "Every coding agent, on every machine, in one place." Hold long enough to
   read, then fade into the app.
2. Ring the session list and point at it with a callout: "Every session on every host, with live status." Hold, then
   remove it.
3. Highlight the waiting session ("migrate auth tokens") with the rest of the screen dimmed, and a callout at its row:
   "This one is waiting on you."
4. Click that session. Its terminal fills the main pane with the agent's question visible. Callout pointing at the
   question: "The agent's real terminal, right in the browser."
5. Click into the terminal and answer the question by pressing 1. The question disappears and the agent goes back to
   work.
6. Cut away the wait while the status catches up, then point at the session's status with a callout: "Status follows the
   agent: working again."
7. When the agent asks for more, show the caption "Type to it like you would locally." and type the follow-up "also
   update the rollout doc", then Enter. Let the reply stream in.
8. End card: "farhelm", with the subtitle "Watch your agents. Answer them from anywhere."
