# Maintainer writing

What a Blocked question, a report and a landing note must say so the maintainer can act on them without opening the
code. Executors and drainers read it before their first action (a block can come at any step), the landing monitor
before its first round, and the cold-read sub-agents before reading the text they check (`plans/AGENTS.md`, "Flows and
reading lists").

## Writing for the maintainer

The maintainer reads Blocked questions and reports without opening the code, without remembering what a PR number stands
for, and without remembering the review finding or triage decision behind the plan. A report is read after its work has
landed, so it is the maintainer's only account of what reached main and what they should know about it. Every decision a
report records and every question it asks must be one the maintainer can take a position on straight away. In product
terms, per `AGENTS.md`, "Talking to the user", state:

- the feature or operation involved and the original problem: what went wrong for a user, under what trigger, and what
  they saw or lost;
- what the maintainer decided about it during triage or in earlier Decisions, if anything;
- what the plan did, and what execution found that the decision did not anticipate;
- for a question: the question itself, the realistic options with their tradeoffs, and a recommendation.

A PR number may follow as a pointer, never in place of that context. Use `###` or deeper headings in a Blocked question;
the script refuses `#` and `##`, which would break the plan file's sections.

A report has these sections, in this order: what this was about; things you should know; open questions and possible
follow-ups; the PRs, one line each; checks run, reused and skipped, with the reason for each; the review gate's outcome.
In a later round, a "since the last review" section comes first and names which PRs changed, were added, or were
dropped.
