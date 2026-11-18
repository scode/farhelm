#!/bin/bash
#
# Send one release notification at ntfy's highest priority, as required every time a release stops to wait on the
# maintainer by `releasing/AGENTS.md`, "Notifying the maintainer". Whether a moment deserves one is the caller's call
# under that section; this only sends.
#
# It goes to the same topic as the plans flow's notifications (`plans/AGENTS.md`, "Notifications"), with its own title
# so the maintainer can tell a waiting release from a waiting plan at a glance. The topic name is the only thing keeping
# the notifications private, so it never reaches any output: not in a message, not in curl's output (which goes to
# /dev/null because ntfy answers with JSON that names the topic), and not in an error, which says only what is wrong
# with the file. Like the plans command, the topic does appear in curl's arguments; development tooling here assumes a
# single-user machine. `-q` keeps a user `.curlrc` (a `verbose` line, say) from logging the request path.
#
# The message must be one line in product terms naming the release, never report text, paths or log excerpts (ntfy.sh
# is a third party). `--data-raw` rather than `-d` keeps a message that starts with `@` from being read as a local file
# to send. Taking the message as an argument lets a caller pass it as a single-quoted literal, so the call needs no
# shell variable or expansion of its own.
#
# The send is bounded (10 seconds to connect, 20 in all) because the caller sends before it asks its question: a send
# that hung would hold the question back, which is the silent stall this exists to prevent.
#
# Usage: releasing/notify.sh MESSAGE
# Exit status: 0 sent; 1 the send failed (curl's error, which never names the topic, is on stderr); 2 usage; 3 the topic
# file does not exist; 4 the topic file does not hold a valid ntfy topic. Nothing is sent on 2 to 4. A failure is for
# the caller to report and log; it does not stop the release.

topic_file="$HOME/.farhelm-ntfy-topic"
msg="$1"
if [ -z "$msg" ] || [ "$#" -ne 1 ]; then
  echo "usage: releasing/notify.sh MESSAGE" >&2
  exit 2
fi
if [ ! -f "$topic_file" ]; then
  echo "notify.sh: ~/.farhelm-ntfy-topic does not exist; no notification sent" >&2
  exit 3
fi
# Same whitespace handling as the plans command, so both send to the same topic. The grammar is ntfy's own (letters,
# digits, underscore and dash, at most 64), checked here so a bad file is reported as such rather than as a curl error.
topic="$(tr -d '[:space:]' < "$topic_file")"
if ! [[ "$topic" =~ ^[A-Za-z0-9_-]{1,64}$ ]]; then
  echo "notify.sh: ~/.farhelm-ntfy-topic does not hold a valid ntfy topic; no notification sent" >&2
  exit 4
fi
if ! curl -q -fsS --connect-timeout 10 --max-time 20 -o /dev/null -H "Title: farhelm release" -H "Priority: urgent" \
  --data-raw "$msg" "https://ntfy.sh/$topic"; then
  exit 1
fi
