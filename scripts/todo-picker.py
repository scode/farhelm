#!/usr/bin/env python3
"""Serve a local checklist page for picking which TODO.md entries to plan.

This is the tool behind the "todo planning flow" in `plans/AGENTS.md`. The maintainer ticks entries from one TODO.md
bucket, clicks "Copy code", and pastes a line like `todo-plan near-term: pin-rust-toolchain, upgrade-epochs` back into
the agent session, which then plans those entries. The page exists because choosing from a long bucket by typing entry
names back is slow and error prone, while ticking boxes is not.

NOTE: This script does not write the summaries. An agent reads each entry and writes a short label and a one-to-two
sentence summary, in product terms, into a JSON file. The script only guarantees that the file covers exactly the
bucket's entries, renders it, and serves it. Here's the TLDR of the two steps:

- `skeleton --bucket <name>` prints a JSON file listing every entry in the bucket that does not already reference a plan
  (`Plan: plans/queue/<slug>.md`), in file order, with empty `label` and `tldr` fields for the agent to fill in.
  Entries that already have a plan are left out, because `plans/AGENTS.md` never plans an entry twice, and reported on
  stderr.
- `serve --summaries <file>` re-reads TODO.md, refuses a summaries file that no longer matches the bucket's unplanned
  entries exactly (an entry added, removed, reworded at its key, or planned since the skeleton), and serves the page on
  `127.0.0.1` until killed.

An entry's key is its bold handle (`- **Handle.** ...`) when it has one, and otherwise its whole first physical line,
since several buckets hold entries without handles. Keys only need to be stable between `skeleton` and `serve`.

The port defaults to 14003 because the maintainer asked for a fixed, memorable URL to keep open in a browser. That is
the same exception `website/scripts/preview.sh` makes; it is not a test harness, so root CLAUDE.md's rule against fixed
ports in harnesses does not apply. When the port is taken the script refuses rather than picking another one, because a
silently different URL would leave the maintainer looking at a stale page. It binds loopback only and serves nothing but
the one page.

Stdlib only, so it runs with any Python 3.9+ without a virtualenv. `scripts/test-todo-picker.py` covers the parsing,
validation and rendering.
"""

from __future__ import annotations

import argparse
import html
import http.server
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

DEFAULT_BUCKET = "Near term"
DEFAULT_PORT = 14003
REPO_ROOT = Path(__file__).resolve().parent.parent

# A plan reference as plans/AGENTS.md has TODO entries spell it. dprint can wrap the line before the backticked path,
# but never inside it, so matching the joined entry text is enough.
PLAN_REF = re.compile(r"Plan: `plans/queue/([^`]+)\.md`")
BOLD_HANDLE = re.compile(r"^\*\*(.+?)\*\*")


class PickerError(Exception):
    """A problem the agent running the flow has to fix (wrong bucket name, stale summaries); printed without a trace."""


@dataclass
class Entry:
    """One top-level bullet of a TODO.md bucket.

    `group` is the `###` subheading the entry sits under, or None directly under the bucket heading. `plan` is the
    referenced plan's slug when the entry already has one. `text` is the entry's physical lines joined with single
    spaces, which is all the parsing needs; it is not a faithful copy of the Markdown.
    """

    key: str
    group: str | None
    text: str
    plan: str | None


# ---- Parsing TODO.md --------------------------------------------------------------------------------------------------


def kebab(text: str) -> str:
    """Lowercase ASCII words joined by hyphens: the form bucket names and entry labels take in the pasted code."""
    return "-".join(re.findall(r"[a-z0-9]+", text.lower()))


def bucket_names(todo_text: str) -> list[str]:
    """Every `## ` heading in TODO.md, in file order."""
    return [line[3:].strip() for line in todo_text.splitlines() if line.startswith("## ")]


def resolve_bucket(todo_text: str, requested: str) -> str:
    """Map a requested bucket name to its exact heading, matching case-insensitively and ignoring punctuation.

    "near term", "Near term" and "near-term" all name the same bucket. An unknown name is an error that lists the real
    ones, since the maintainer names buckets from memory.
    """
    names = bucket_names(todo_text)
    for name in names:
        if kebab(name) == kebab(requested):
            return name
    raise PickerError(f"no bucket named {requested!r} in TODO.md; buckets are: {', '.join(names)}")


def parse_bucket(todo_text: str, bucket: str) -> list[Entry]:
    """Return the top-level entries of one bucket, in file order, including ones that already have a plan.

    The bucket runs from its `## ` heading to the next `## ` heading; `###` subheadings inside it become the entries'
    groups. An entry starts at a `- ` line in column 0 and continues over the indented lines below it. Anything else
    (the bucket's intro paragraphs, blank lines) is ignored, and so are nested bullets, which belong to their parent.
    """
    entries: list[Entry] = []
    in_bucket = False
    group: str | None = None
    current: list[str] | None = None
    current_group: str | None = None

    def flush() -> None:
        nonlocal current
        if current is None:
            return
        text = " ".join(part.strip() for part in current)
        first = current[0].strip()
        handle = BOLD_HANDLE.match(first)
        key = handle.group(1) if handle else first
        plan = PLAN_REF.search(text)
        entries.append(Entry(key=key, group=current_group, text=text, plan=plan.group(1) if plan else None))
        current = None

    for line in todo_text.splitlines():
        if line.startswith("## "):
            flush()
            in_bucket = line[3:].strip() == bucket
            group = None
            continue
        if not in_bucket:
            continue
        if line.startswith("### "):
            flush()
            group = line[4:].strip()
        elif line.startswith("- "):
            flush()
            current = [line[2:]]
            current_group = group
        elif current is not None and line.startswith("  "):
            current.append(line)
        elif current is not None and line.strip() == "":
            continue
        else:
            # A non-indented, non-bullet line (an intro paragraph after the entries, say) ends the entry.
            flush()
    flush()

    keys = [entry.key for entry in entries]
    duplicates = sorted({key for key in keys if keys.count(key) > 1})
    if duplicates:
        raise PickerError(f"bucket {bucket!r} has entries sharing a key, so they cannot be told apart: {duplicates}")
    return entries


# ---- Summaries file ---------------------------------------------------------------------------------------------------


def skeleton(todo_text: str, requested_bucket: str) -> tuple[dict, list[Entry]]:
    """Build the summaries file for the agent to fill in, and return the entries left out because they have a plan.

    The `text` field is there so the agent can write each summary from the skeleton alone; `serve` ignores it.
    """
    bucket = resolve_bucket(todo_text, requested_bucket)
    entries = parse_bucket(todo_text, bucket)
    planned = [entry for entry in entries if entry.plan]
    doc = {
        "bucket": bucket,
        "entries": [
            {"key": entry.key, "group": entry.group, "text": entry.text, "label": "", "tldr": ""}
            for entry in entries
            if not entry.plan
        ],
    }
    return doc, planned


@dataclass
class Item:
    """One checkbox on the page: a validated summaries entry plus the slug the pasted code uses for it."""

    slug: str
    group: str | None
    label: str
    tldr: str


def validate(todo_text: str, doc: dict) -> tuple[str, list[Item]]:
    """Check a filled-in summaries file against the current TODO.md and return the bucket heading and page items.

    The file must name an existing bucket and list exactly that bucket's unplanned entry keys, in file order, each
    with a non-empty label and summary. Matching exactly is the point: a summaries file written against an older
    TODO.md would otherwise offer entries that were removed or already planned, or silently leave out new ones. Labels
    must produce distinct slugs, because the pasted code names entries by slug.
    """
    bucket = resolve_bucket(todo_text, str(doc.get("bucket", "")))
    expected = [entry for entry in parse_bucket(todo_text, bucket) if not entry.plan]
    got = doc.get("entries")
    if not isinstance(got, list):
        raise PickerError("summaries file has no 'entries' list")

    expected_keys = [entry.key for entry in expected]
    got_keys = [item.get("key") if isinstance(item, dict) else None for item in got]
    if got_keys != expected_keys:
        missing = [key for key in expected_keys if key not in got_keys]
        extra = [key for key in got_keys if key not in expected_keys]
        detail = f"missing {missing}, unexpected {extra}" if missing or extra else "same keys in a different order"
        raise PickerError(
            f"summaries no longer match the {bucket!r} bucket ({detail}); rerun skeleton and fill it in again"
        )

    items: list[Item] = []
    for entry, raw in zip(expected, got):
        label = str(raw.get("label", "")).strip()
        tldr = str(raw.get("tldr", "")).strip()
        if not label or not tldr:
            raise PickerError(f"entry {entry.key!r} needs both a label and a tldr")
        items.append(Item(slug=kebab(label), group=entry.group, label=label, tldr=tldr))

    slugs = [item.slug for item in items]
    clashes = sorted({slug for slug in slugs if slugs.count(slug) > 1 or not slug})
    if clashes:
        raise PickerError(f"labels must give distinct, non-empty slugs; clashing: {clashes}")
    return bucket, items


# ---- Rendering --------------------------------------------------------------------------------------------------------


def inline(text: str) -> str:
    """Escape text for HTML, turning backtick spans into `<code>` so summaries can name paths and commands."""
    parts = text.split("`")
    out = []
    for index, part in enumerate(parts):
        escaped = html.escape(part, quote=False)
        # Odd-indexed parts sit after an opening backtick. Only the last part can lack a closing one (an odd number of
        # backticks), and that stray backtick is kept as literal text rather than opening a code span that never ends.
        if index % 2 == 1 and index < len(parts) - 1:
            out.append(f"<code>{escaped}</code>")
        elif index % 2 == 1:
            out.append("`" + escaped)
        else:
            out.append(escaped)
    return "".join(out)


# The palette and type scale follow the maintainer's snippet stylepack, so the page reads like the snippets they are
# used to. The fonts are left to the system: the page is ephemeral and local, and vendoring Inter here is not worth it.
PAGE_CSS = """
:root {
  --bg: #f3f5f6; --surface: #ffffff; --fg: #1b2530; --muted: #5a6875; --rule: #d4dbe0;
  --accent: #1f6f8b; --accent-soft: #dcecf2;
  --font-body: system-ui, -apple-system, "Segoe UI", sans-serif;
  --font-mono: ui-monospace, Menlo, Consolas, monospace;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0f161d; --surface: #16212b; --fg: #e3e9ee; --muted: #93a3b1; --rule: #2a3a47;
    --accent: #6cc0dc; --accent-soft: #173341;
    color-scheme: dark;
  }
}
*, *::before, *::after { box-sizing: border-box; }
body { margin: 0; padding: 0 16px; background: var(--bg); color: var(--fg); font: 16px/1.65 var(--font-body); }
main { max-width: 102ch; margin: 0 auto; padding-block: 48px 96px; }
h1 { font-size: 2.6rem; line-height: 1.2; letter-spacing: -0.025em; margin: 0; }
h2 { font-size: 1.35rem; margin: 1.6em 0 .4em; }
.eyebrow { font: 600 .75rem/1 var(--font-mono); text-transform: uppercase; letter-spacing: .12em; color: var(--accent); }
.lede { font-size: 1.2rem; color: var(--muted); font-style: italic; }
code, pre { font-family: var(--font-mono); font-size: .86em; }
:not(pre) > code { background: var(--accent-soft); padding: .1em .35em; border-radius: 4px; }
pre { background: var(--surface); border: 1px solid var(--rule); border-radius: 6px; padding: 14px 16px;
  white-space: pre-wrap; overflow-wrap: anywhere; }
ul.tasks { list-style: none; padding-left: .2em; }
ul.tasks li { display: flex; gap: .6em; align-items: flex-start; }
ul.tasks li + li { margin-top: .7em; }
ul.tasks input { accent-color: var(--accent); margin-top: .4em; flex: none; }
.actions { display: flex; gap: .8em; align-items: center; flex-wrap: wrap; margin-top: 1.5em; }
.actions button { font: inherit; padding: .4em 1em; border-radius: 6px; cursor: pointer;
  border: 1px solid var(--accent); background: var(--accent); color: var(--surface); }
.actions button.secondary { background: transparent; color: var(--accent); }
button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
#status { color: var(--muted); }
"""

# The code lists slugs in page order whatever order the boxes were ticked in, so a selection always produces the same
# text. It is shown on the page as well as copied, because the clipboard API can refuse and the maintainer can still
# select the text by hand. The textarea fallback covers browsers that withhold navigator.clipboard outside a secure
# context, which happens when the page is reached through a non-loopback name.
PAGE_JS = """
const prefix = document.body.dataset.prefix;
const boxes = [...document.querySelectorAll('ul.tasks input[type=checkbox]')];
const codeEl = document.getElementById('code');
const statusEl = document.getElementById('status');
function currentCode() {
  const picked = boxes.filter(b => b.checked).map(b => b.value);
  return picked.length ? prefix + ' ' + picked.join(', ') : '';
}
function render() { codeEl.textContent = currentCode() || '(nothing selected)'; statusEl.textContent = ''; }
async function copy(text) {
  try { await navigator.clipboard.writeText(text); return true; }
  catch (_) {
    const ta = document.createElement('textarea');
    ta.value = text; document.body.appendChild(ta); ta.select();
    const ok = document.execCommand('copy'); ta.remove(); return ok;
  }
}
boxes.forEach(b => b.addEventListener('change', render));
document.getElementById('copy').addEventListener('click', async () => {
  const code = currentCode();
  if (!code) { statusEl.textContent = 'Tick at least one entry first.'; return; }
  statusEl.textContent = (await copy(code)) ? 'Copied.' : 'Copy failed; select the code below by hand.';
});
document.getElementById('clear').addEventListener('click', () => { boxes.forEach(b => { b.checked = false; }); render(); });
render();
"""


def render_page(bucket: str, items: list[Item]) -> str:
    """The whole page as one self-contained HTML document; the pasted code starts with `todo-plan <bucket-slug>:`."""
    prefix = f"todo-plan {kebab(bucket)}:"
    body: list[str] = []
    open_group: object = object()  # sentinel that no real group equals, so the first item always opens a list
    for item in items:
        if item.group != open_group:
            if body:
                body.append("</ul>")
            if item.group:
                body.append(f"<h2>{html.escape(item.group)}</h2>")
            body.append('<ul class="tasks">')
            open_group = item.group
        slug = html.escape(item.slug)
        body.append(
            f'<li><input type="checkbox" id="{slug}" value="{slug}"><label for="{slug}">'
            f"<strong>{inline(item.label)}.</strong> {inline(item.tldr)}</label></li>"
        )
    if body:
        body.append("</ul>")
    else:
        body.append("<p>Every entry in this bucket already has a plan.</p>")

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>TODO planning picks</title>
<style>{PAGE_CSS}</style>
</head>
<body data-prefix="{html.escape(prefix)}">
<main>
<p class="eyebrow">Farhelm TODO.md · {html.escape(bucket)}</p>
<h1>What should get planned?</h1>
<p class="lede">Tick the entries to plan, copy the code, and paste it back into the session.
Entries that already have a plan are not listed.</p>
{chr(10).join(body)}
<div class="actions">
<button type="button" id="copy">Copy code</button>
<button type="button" class="secondary" id="clear">Clear</button>
<span id="status" aria-live="polite"></span>
</div>
<p>Code to paste back:</p>
<pre><code id="code"></code></pre>
</main>
<script>{PAGE_JS}</script>
</body>
</html>
"""


# ---- Serving ----------------------------------------------------------------------------------------------------------


def serve(page: str, port: int) -> None:
    """Serve the page at `/` on loopback until interrupted; every other path is a 404."""
    payload = page.encode("utf-8")

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:  # noqa: N802 (http.server's naming)
            if self.path not in ("/", "/index.html"):
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, format: str, *args: object) -> None:  # noqa: A002
            pass

    try:
        server = http.server.HTTPServer(("127.0.0.1", port), Handler)
    except OSError as err:
        raise PickerError(f"cannot listen on 127.0.0.1:{port} ({err.strerror}); something else holds it") from err
    print(f"serving http://127.0.0.1:{port}/", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--todo", type=Path, default=REPO_ROOT / "TODO.md", help="TODO.md to read (default: the repo's)")
    sub = parser.add_subparsers(dest="command", required=True)
    sk = sub.add_parser("skeleton", help="print the summaries file to fill in")
    sk.add_argument("--bucket", default=DEFAULT_BUCKET, help=f"bucket heading (default: {DEFAULT_BUCKET!r})")
    sv = sub.add_parser("serve", help="validate a filled-in summaries file and serve the page")
    sv.add_argument("--summaries", type=Path, required=True)
    sv.add_argument("--port", type=int, default=DEFAULT_PORT)
    args = parser.parse_args(argv)

    try:
        todo_text = args.todo.read_text(encoding="utf-8")
        if args.command == "skeleton":
            doc, planned = skeleton(todo_text, args.bucket)
            for entry in planned:
                print(f"skipped, already planned in plans/queue/{entry.plan}.md: {entry.key}", file=sys.stderr)
            json.dump(doc, sys.stdout, indent=2, ensure_ascii=False)
            sys.stdout.write("\n")
        else:
            doc = json.loads(args.summaries.read_text(encoding="utf-8"))
            bucket, items = validate(todo_text, doc)
            serve(render_page(bucket, items), args.port)
    except (PickerError, OSError, json.JSONDecodeError) as err:
        print(f"todo-picker: {err}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
