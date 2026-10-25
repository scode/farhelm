#!/usr/bin/env python3
"""Render a legacy standalone release-note review page.

Stable releases now use the `interactive-artifact` skill (`releasing/.agents/stable.md`, "Cutting a stable release",
step 2). Do not use this generator for that flow: its full document and scripts are incompatible with the skill's body
fragments. The remaining description covers the standalone page retained here.

The proposed `## vX.Y.Z` section lives in a scratch Markdown file, exactly as it will land in `CHANGELOG.md`. This
script turns that file into the page. Every entry gets a short id (the category's initial and its position: `B2`,
`A14`, `M3`) and a note box under it, plus one box for general notes. **Copy feedback** gathers the non-empty notes into
one plain-text block that the maintainer pastes back into the agent session:

```
notes-feedback v0.29.0

[A2] Choose Pin to top in a session's menu to keep it above your other ses…
> just use the first sentence
```

Each note carries its entry's opening words as well as its id, because ids are positions and shift when an entry is
added, moved or removed between rounds; the agent matches on the words. The block is also shown on the page, since the
clipboard API can refuse.

Notes are kept in the viewer's localStorage so a reload does not lose them, and each note remembers the revision of the
page it was typed on. The revision is a hash of the section's text, so a page regenerated after a round of edits has a
new one, and loading it drops every note from an earlier revision: those are the notes the agent just applied, and
leaving them in the boxes would invite pasting them a second time. Notes typed on the current revision stay. The
maintainer asked for this on 2026-10-16, when the first version kept applied notes around after an update.

Only the Markdown subset the changelog format allows is handled (`releasing/.agents/changelog.md`, "Format"): one `##`
release heading, `###` categories, `-` bullets, the italic Misc fixes paragraph, inline code, bold, italics, links, and
trailing `(#N)` PR references, which become links to each PR's diff.

The page is one self-contained file with its stylesheet inline, so it looks the same wherever it is served from. It used
to be published as a scode-snippets snippet and linked that repository's `default1` stylepack by relative path, which
resolves nowhere else; the maintainer moved the review to a locally served page on 2026-10-23.

What this does NOT do: serve the page, parse the copied feedback, or check the section against the changelog format
(that is `releasing/check-changelog.py format`, which runs on `CHANGELOG.md`).

Usage:
  notes-review-page.py SECTION.md OUT.html
  notes-review-page.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit

PR_URL = "https://github.com/scode/farhelm/pull/"

# Category name (the heading after its emoji) to id prefix. The initials are unique across the current category set.
PREFIX = {
    "Breaking": "B",
    "Added": "A",
    "Changed": "C",
    "Fixes worth highlighting": "H",
    "Removed": "R",
    "Misc fixes": "M",
}

QUOTE_LIMIT = 70


# ---- Markdown to HTML -------------------------------------------------------------------------------------------------


def inline_link(match: re.Match) -> str:
    """Keep draft links inert unless their target is HTTP(S) or relative.

    Inline text is already escaped; decode that layer before URL parsing and
    escape again for an attribute, where quotes have a different meaning.
    """
    # Browsers ignore leading C0 bytes and spaces. Normalize explicitly so
    # the scheme check does not depend on the Python URL parser's version.
    target = re.sub(r"^[\x00-\x20]+", "", html.unescape(match.group(2)))
    try:
        scheme = urlsplit(target).scheme.lower()
    except ValueError:
        return match.group(1)
    if scheme not in ("", "http", "https"):
        return match.group(1)
    return f'<a href="{html.escape(target, quote=True)}">{match.group(1)}</a>'


def inline(text: str) -> str:
    """Convert one line of inline Markdown to HTML, escaping everything else.

    Code spans are cut out first so `*` or `<U+200B>` inside them stay literal.
    """
    out = []
    for part in re.split(r"(`[^`]*`)", text):
        if len(part) >= 2 and part.startswith("`") and part.endswith("`"):
            out.append("<code>" + html.escape(part[1:-1]) + "</code>")
            continue
        s = html.escape(part, quote=False)
        s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
        s = re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
        s = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", inline_link, s)
        out.append(s)

    def link_refs(m: re.Match) -> str:
        refs = [f'<a href="{PR_URL}{n}/changes">#{n}</a>' for n in re.findall(r"#(\d+)", m.group(1))]
        return "(" + ", ".join(refs) + ")"

    # PR references are only the trailing "(#N, #M)" group of an entry.
    return re.sub(r"\((#\d+(?:, #\d+)*)\)\s*$", link_refs, "".join(out))


def opening_words(text: str) -> str:
    """The plain-text start of an entry, quoted in the copied feedback so a note survives renumbering."""
    plain = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    plain = re.sub(r"[`*]", "", plain)
    plain = re.sub(r"\s*\((#\d+(?:, #\d+)*)\)\s*$", "", plain)
    return plain if len(plain) <= QUOTE_LIMIT else plain[: QUOTE_LIMIT - 1].rstrip() + "…"


def revision(md: str) -> str:
    """The page revision: changes whenever the section's text does, so applied notes can be told apart."""
    return hashlib.sha256(md.encode("utf-8")).hexdigest()[:12]


# ---- Page -------------------------------------------------------------------------------------------------------------

# The page's whole stylesheet: a served local page cannot reach a stylesheet outside its own directory, so nothing is
# linked. The palette and type scale are `scripts/todo-picker.py`'s, which follow the maintainer's snippet stylepack, so
# the local review pages look alike and like the snippets the maintainer is used to. Fonts are left to the system, as
# there: the page is ephemeral and local.
STYLE = """
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
  h1 { font-size: 2.6rem; line-height: 1.2; letter-spacing: -0.025em; margin: 0.3em 0 0; }
  h2 { font-size: 1.35rem; margin: 1.6em 0 0.4em; }
  a { color: var(--accent); }
  .eyebrow { font: 600 0.75rem/1 var(--font-mono); text-transform: uppercase; letter-spacing: 0.12em;
             color: var(--accent); }
  .lede { font-size: 1.2rem; color: var(--muted); font-style: italic; }
  code, pre { font-family: var(--font-mono); font-size: 0.86em; }
  :not(pre) > code { background: var(--accent-soft); padding: 0.1em 0.35em; border-radius: 4px; }
  pre { background: var(--surface); border: 1px solid var(--rule); border-radius: 6px; padding: 14px 16px;
        white-space: pre-wrap; overflow-wrap: anywhere; }
  .entries li { margin-bottom: 0.9em; }
  .eid { font-family: var(--font-mono); font-size: 0.8em; color: var(--muted);
         border: 1px solid var(--rule); border-radius: 4px; padding: 0 0.3em; margin-right: 0.2em; }
  .note { display: block; width: 100%; box-sizing: border-box; margin-top: 0.35em; font: inherit;
          font-size: 0.9em; padding: 0.3em 0.5em; border: 1px dashed var(--rule); border-radius: 6px;
          background: var(--surface); color: inherit; resize: vertical; }
  .note.has-text { border-style: solid; border-color: var(--accent); }
  #general { min-height: 4em; }
  .toolbar { display: flex; gap: 0.6em; align-items: center; flex-wrap: wrap; margin: 1em 0; }
  .toolbar button { font: inherit; padding: 0.35em 0.9em; border-radius: 6px; border: 1px solid var(--accent);
                    background: var(--accent); color: var(--bg); cursor: pointer; }
  .toolbar button.secondary { background: transparent; color: var(--accent); }
"""

# The page script. __RELEASE__ and __REVISION__ are replaced with JSON string literals. Stored values are
# `{"rev": ..., "text": ...}`; anything else (an older plain-string value, or unparseable JSON) is dropped like a note
# from another revision.
SCRIPT = """
const RELEASE = __RELEASE__;
const REVISION = __REVISION__;
const KEY = 'farhelm-notes-review:' + RELEASE + ':';
const store = {
  keys() { try { return Object.keys(localStorage).filter(k => k.startsWith(KEY)); } catch (e) { return []; } },
  read(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
  drop(k) { try { localStorage.removeItem(k); } catch (e) {} },
  save(id, text) {
    try {
      if (text) localStorage.setItem(KEY + id, JSON.stringify({ rev: REVISION, text }));
      else localStorage.removeItem(KEY + id);
    } catch (e) {}
  },
};
const boxes = [...document.querySelectorAll('li[data-id] .note')];
const general = document.getElementById('general');
const codeEl = document.getElementById('code');
const statusEl = document.getElementById('status');
const idOf = ta => ta === general ? 'general' : ta.closest('li').dataset.id;

// Drop notes typed on an earlier revision of the page: the agent applied them before regenerating it.
let cleared = 0;
const kept = {};
for (const k of store.keys()) {
  let v = null;
  try { v = JSON.parse(store.read(k)); } catch (e) {}
  if (v && v.rev === REVISION && typeof v.text === 'string') kept[k.slice(KEY.length)] = v.text;
  else { store.drop(k); cleared++; }
}

// Grow a note box to fit its text so long feedback stays readable.
function fit(ta) {
  ta.style.height = 'auto';
  ta.style.height = ta.scrollHeight + 'px';
  ta.classList.toggle('has-text', ta.value.trim() !== '');
}

function build() {
  const lines = ['notes-feedback ' + RELEASE];
  for (const ta of boxes) {
    const v = ta.value.trim();
    if (!v) continue;
    const li = ta.closest('li');
    lines.push('', '[' + li.dataset.id + '] ' + li.dataset.quote);
    for (const l of v.split('\\n')) lines.push('> ' + l);
  }
  const g = general.value.trim();
  if (g) { lines.push('', '[general]'); for (const l of g.split('\\n')) lines.push('> ' + l); }
  return lines.length > 1 ? lines.join('\\n') : '';
}
function refresh() { codeEl.textContent = build() || '(no feedback yet)'; }

for (const ta of [...boxes, general]) {
  const id = idOf(ta);
  ta.value = kept[id] || '';
  fit(ta);
  ta.addEventListener('input', () => { store.save(id, ta.value); fit(ta); refresh(); });
}
refresh();
if (cleared) statusEl.textContent = 'This draft was updated; feedback from the previous version was cleared.';

// The clipboard API can be withheld; fall back to a hidden textarea, and the block below stays selectable by hand.
async function copy(text) {
  try { await navigator.clipboard.writeText(text); return true; }
  catch (e) {
    const ta = document.createElement('textarea'); ta.value = text; document.body.appendChild(ta); ta.select();
    const ok = document.execCommand('copy'); ta.remove(); return ok;
  }
}
document.getElementById('copy').addEventListener('click', async () => {
  const text = build();
  if (!text) { statusEl.textContent = 'Nothing to copy yet.'; return; }
  statusEl.textContent = (await copy(text)) ? 'Copied.' : 'Copy failed; select the text below by hand.';
});
document.getElementById('clear').addEventListener('click', () => {
  if (!confirm('Clear every note on this page?')) return;
  for (const ta of [...boxes, general]) { ta.value = ''; store.save(idOf(ta), ''); fit(ta); }
  refresh();
  statusEl.textContent = 'Cleared.';
});
"""


def render(md: str) -> str:
    """The complete review page for one release section."""
    body = []
    in_list = False
    title = "release notes"
    prefix = "X"
    counter = 0
    for line in md.splitlines():
        if line.startswith("- "):
            if not in_list:
                body.append('  <ul class="entries">')
                in_list = True
            counter += 1
            eid = f"{prefix}{counter}"
            quote = html.escape(opening_words(line[2:]), quote=True)
            body.append(
                f'    <li data-id="{eid}" data-quote="{quote}"><span class="eid">{eid}</span> {inline(line[2:])}'
                f'<textarea class="note" rows="1" placeholder="feedback on {eid}" aria-label="feedback on {eid}">'
                "</textarea></li>"
            )
            continue
        if in_list:
            body.append("  </ul>")
            in_list = False
        if line.startswith("## "):
            title = line[3:]
            body.append("  <h1>" + inline(title) + "</h1>")
        elif line.startswith("### "):
            heading = line[4:]
            name = heading.split(" ", 1)[1] if " " in heading else heading
            prefix = PREFIX.get(name, name[:1].upper())
            counter = 0
            body.append("  <h2>" + inline(heading) + "</h2>")
        elif line.strip():
            body.append("  <p>" + inline(line) + "</p>")
    if in_list:
        body.append("  </ul>")

    version = title.split(" ")[0]
    script = SCRIPT.replace("__RELEASE__", json.dumps(version)).replace("__REVISION__", json.dumps(revision(md)))
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Farhelm {html.escape(version)} release notes draft</title>
<style>{STYLE}</style>
</head>
<body>
<main>
  <p class="eyebrow">Draft for review, not published notes</p>
  <p class="lede">Type feedback under any entry, then press <strong>Copy feedback</strong> and paste the result into
  the session. Notes stay in this browser across reloads until the draft is updated.</p>
{chr(10).join(body)}
  <h2>General notes</h2>
  <textarea id="general" class="note" placeholder="anything not tied to one entry, or &quot;approved&quot;"
    aria-label="general notes"></textarea>
  <div class="toolbar">
    <button type="button" id="copy">Copy feedback</button>
    <button type="button" id="clear" class="secondary">Clear notes</button>
    <span id="status" role="status"></span>
  </div>
  <pre class="wrap"><code id="code"></code></pre>
</main>
<script>{script}</script>
</body>
</html>
"""


# ---- Self-test --------------------------------------------------------------------------------------------------------

FIXTURE = """## v9.9.0 - 2026-01-01

**DRAFT release notes - not yet finalized**

### 💥 Breaking

- This release *requires* you to update your remote hosts. (#1, #2)

### 🚀 Added

- Codes such as `<U+200B>` and [ntfy](https://ntfy.sh/) stay literal & linked. (#3)
- A second entry whose opening words run on well past the quote limit so that the copied feedback has to cut them off. (#4)

### 🩹 Misc fixes

*Fixes that would normally be summed up as a single "various bug fixes" line.*

- A misc fix. (#5)
"""

# Drives the page script under node with a fake DOM and a fake localStorage that persists between "page loads", to
# check the revision rule: notes typed on revision r1 come back on a reload of r1 and are dropped by r2. The page script
# runs in the same module scope, so harness names must not collide with its own (`store`, `codeEl`, ...): they are
# prefixed `fake`/`backing`. The page enumerates notes with Object.keys(localStorage), as a browser allows; a plain object
# stand-in would list its methods instead, so Object.keys is routed to the backing map for it.
NODE_HARNESS = """
const backing = new Map(Object.entries(JSON.parse(process.argv[2] || '{}')));
globalThis.localStorage = {
  getItem: k => backing.has(k) ? backing.get(k) : null,
  setItem: (k, v) => backing.set(k, String(v)),
  removeItem: k => backing.delete(k),
};
const realKeys = Object.keys;
Object.keys = o => o === globalThis.localStorage ? [...backing.keys()] : realKeys(o);
const listeners = {};
function mkTa(id, quote) {
  const li = id ? { dataset: { id, quote } } : null;
  return { value: '', style: {}, classList: { toggle() {} }, scrollHeight: 10, closest: () => li,
           addEventListener(ev, fn) { (listeners[id || 'general'] ||= []).push(fn); } };
}
const a = mkTa('B1', 'first entry'), b = mkTa('A1', 'second entry'), g = mkTa(null);
const fakeCode = { textContent: '' }, fakeStatus = { textContent: '' };
globalThis.document = {
  querySelectorAll: () => [a, b],
  getElementById: id => ({ general: g, code: fakeCode, status: fakeStatus,
                           copy: { addEventListener() {} }, clear: { addEventListener() {} } })[id],
};
__SCRIPT__
const action = process.argv[3];
if (action === 'type') {
  a.value = 'reword this\\nplease'; listeners['B1'].forEach(f => f());
  g.value = 'approved otherwise'; listeners['general'].forEach(f => f());
}
console.log(JSON.stringify({ code: fakeCode.textContent, status: fakeStatus.textContent, a: a.value, g: g.value,
                             store: Object.fromEntries(backing) }));
"""


def _run_page(page: str, stored: dict, action: str, workdir: Path) -> dict:
    """Load `page`'s script once under node with `stored` as localStorage; return what the fake DOM ended up showing."""
    script = re.search(r"<script>(.*?)</script>", page, re.S).group(1)
    path = workdir / "page.mjs"
    path.write_text(NODE_HARNESS.replace("__SCRIPT__", script), encoding="utf-8")
    run = subprocess.run(["node", str(path), json.dumps(stored), action], capture_output=True, text=True)
    if run.returncode != 0:
        raise RuntimeError(f"node exited {run.returncode} running the page script:\n{run.stderr}")
    return json.loads(run.stdout)


def self_test() -> int:
    """Draft markup must stay inert while review notes survive only their revision.

    Exact link output distinguishes attribute escaping from scheme refusal;
    the Node fixture separately exercises the unchanged feedback lifecycle.
    """
    failures: list[str] = []

    def check(cond: bool, what: str) -> None:
        if not cond:
            failures.append(what)

    page = render(FIXTURE)
    check('data-id="B1"' in page and 'data-id="A2"' in page and 'data-id="M1"' in page, "ids per category")
    check("<code>&lt;U+200B&gt;</code>" in page, "code spans are escaped literally")
    check('<a href="https://ntfy.sh/">ntfy</a>' in page, "inline links are kept")
    for target in ("http://example.invalid/", "https://example.invalid/?a=1&b=2", "../notes", "#entry"):
        check(inline(f"[safe]({target})") == f'<a href="{html.escape(target)}">safe</a>',
              f"allowed link target: {target!r}")
    check(inline('[quoted](https://example.invalid/" onmouseover="evil)')
          == '<a href="https://example.invalid/&quot; onmouseover=&quot;evil">quoted</a>',
          "link targets cannot introduce attributes")
    for target in ("javascript:evil", "JaVaScRiPt:evil", "\tjava\nscript:evil", "\x01javascript:evil",
                   "data:text/html,evil", "file:///x", "https://["):
        check(inline(f"[blocked]({target})") == "blocked", f"inert link target: {target!r}")
    check("&amp; linked" in page, "plain text is escaped")
    check(f'<a href="{PR_URL}1/changes">#1</a>, <a href="{PR_URL}2/changes">#2</a>' in page, "PR references link")
    check("<em>requires</em>" in page, "italics")
    check('data-quote="Codes such as &lt;U+200B&gt; and ntfy stay literal &amp; linked."' in page, "quote is plain")
    long_quote = re.search(r'data-id="A2" data-quote="([^"]*)"', page).group(1)
    check(long_quote.endswith("…") and len(html.unescape(long_quote)) == QUOTE_LIMIT, "long quotes are cut")
    check(revision(FIXTURE) != revision(FIXTURE + "x"), "revision follows the text")
    check(revision(FIXTURE) in page, "page carries its revision")
    # The page is served on its own from a scratch directory, where a linked stylesheet or script would 404.
    check("<link" not in page and "<script src" not in page, "the page loads nothing from outside itself")
    check("--accent:" in page, "the page carries its own palette")

    if shutil.which("node") is None:
        print("SKIPPED: node not on PATH; the page script's reload behavior was not exercised")
    else:
        with tempfile.TemporaryDirectory() as d:
            work = Path(d)
            typed = _run_page(page, {}, "type", work)
            check("[B1] first entry\n> reword this\n> please" in typed["code"], "copied block carries the note")
            check("[general]\n> approved otherwise" in typed["code"], "copied block carries general notes")
            check("[A1]" not in typed["code"], "empty notes stay out of the block")
            reload_same = _run_page(page, typed["store"], "load", work)
            check(reload_same["a"] == "reword this\nplease", "a reload of the same revision keeps notes")
            check(reload_same["status"] == "", "no cleared notice on the same revision")
            updated = render(FIXTURE.replace("A misc fix.", "A reworded misc fix."))
            reload_new = _run_page(updated, typed["store"], "load", work)
            check(reload_new["a"] == "" and reload_new["g"] == "", "a new revision drops earlier notes")
            check(reload_new["store"] == {}, "dropped notes leave storage")
            check("cleared" in reload_new["status"], "the page says it cleared notes")
            legacy = {"farhelm-notes-review:v9.9.0:B1": "an old plain-string note"}
            check(_run_page(page, legacy, "load", work)["a"] == "", "unversioned notes are dropped")

    for line in failures:
        print(f"self-test FAILED: {line}", file=sys.stderr)
    if failures:
        return 1
    print("== self-test ok")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("section", nargs="?", type=Path)
    parser.add_argument("out", nargs="?", type=Path)
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.section is None or args.out is None:
        parser.error("SECTION.md and OUT.html are required unless --self-test is given")
    args.out.write_text(render(args.section.read_text(encoding="utf-8")), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
