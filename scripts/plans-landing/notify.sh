#!/bin/bash
#
# Send one plans notification: the command plans/AGENTS.md, "Notifications", gives, with the message as an argument.
#
# --urgent adds ntfy's highest priority ("Priority: urgent", level 5), for a true blocker when the maintainer has asked
# for those to stand out. Whether a condition deserves a notification at all, and at which priority, is the caller's
# call under plans/AGENTS.md; this only sends.
#
# The topic file is read by the shell inside the URL and nowhere else: never printed, logged or echoed, and curl's
# output goes to /dev/null because ntfy answers with JSON that names the topic. The message must be one line in
# product terms that names the plan's slug, never report text, paths or log excerpts (ntfy.sh is a third party).
#
# Usage: scripts/plans-landing/notify.sh [--urgent] MESSAGE
# Exit status: curl's (a failed send is for the caller to record in its log; it does not stop a flow), 2 on usage.

priority=()
if [ "$1" = --urgent ]; then
  priority=(-H "Priority: urgent")
  shift
fi
msg="$1"
if [ -z "$msg" ] || [ "$#" -ne 1 ]; then
  echo "usage: notify.sh [--urgent] MESSAGE" >&2
  exit 2
fi
curl -fsS -o /dev/null -H "Title: farhelm plans" "${priority[@]}" --data-raw "$msg" "https://ntfy.sh/$(tr -d '[:space:]' < ~/.farhelm-ntfy-topic)"
