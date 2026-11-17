"""Read the docs-media scenario files, which are JSON5, with nothing but the standard library.

NOTE: This is not a full JSON5 parser. It reads the subset of JSON5 that the repository's scenario files use
(`docs/readme-hero/scenario.json5`, `docs/readme-video/scenario.json5` and `docs/docs-shots/scenario.json5`). On top of
strict JSON, that subset is:

- `//` line comments, anywhere whitespace may go except between an unquoted key and its colon;
- unquoted object keys: an ASCII letter, `_` or `$`, then letters, digits, `_` or `$`;
- a trailing comma after the last element, before the closing `}` or `]`.

Everything else is strict JSON and is judged by `json.loads`, so other JSON5 features (single-quoted strings, block
comments, `Infinity` and `NaN`, hex numbers, JSON5's extra string escapes and line continuations) fail with json's
message at the file's line. A duplicated key fails too, although JSON5 would keep the last one: in a scenario it is a
mistake that silently drops a field. When a scenario needs another feature, add it here with its test in
`test-scenario-json5.py`.

Why this exists: the capture flows used to read the scenarios through the `json5` npm package under `e2e/`, which went
with the browser UI. `publish-readme-hero.sh` reads the hero's size through this module, and the capture flows being
rebuilt on the desktop app, in Python, will use it too, so that the capture and the publish check cannot disagree about
what a scenario says. It returns the document as plain Python values and does not interpret it: no `$USER` expansion,
no field validation.

Callers run under `python3 -I`, which puts no script directory on `sys.path`, so a caller appends `scripts/` to the path
itself (`publish-readme-hero.sh` shows how) and sets `sys.dont_write_bytecode` first, so that the import leaves no
`__pycache__` in the checkout.
"""

import json
from pathlib import Path
import re

# One token of the subset's syntax per alternative, rewritten by `_rewrite`. The string alternative comes first, so
# that nothing inside a string is ever read as syntax. The rest: an opening bracket whose first element is a comma
# (copied as it is, so that json refuses `[,]` and `{,}`, as JSON5 does, instead of the comma being dropped as
# trailing), a line comment, an unquoted key, and a comma with only whitespace and comments before a closing bracket.
# Dropping a comment stops before its newline and nothing else removes one, so json's line numbers are the file's.
_TOKEN = re.compile(
    r'"(?:[^"\\\n]|\\.)*"'
    r"|[\[{](?:\s|//[^\n]*\n)*,"
    r"|//[^\n]*"
    r"|[A-Za-z_$][\w$]*(?=\s*:)"
    r"|,(?=(?:\s|//[^\n]*\n)*[}\]])"
)


class ScenarioSyntaxError(ValueError):
    """A scenario file that is not in the supported subset, or not valid at all.

    The message reads `SOURCE:LINE: what is wrong`, so a caller can print it as it is. `line` is None, and left out of
    the message, for a duplicate key or a `NaN` or `Infinity`, which the JSON decoder reports without a position.
    """

    def __init__(self, source, line, message):
        super().__init__(f"{source}:{line}: {message}" if line is not None else f"{source}: {message}")
        self.line = line


def load(path):
    """Parse the scenario file at `path` and return its document (a dict for every real scenario)."""
    path = Path(path)
    return loads(path.read_text(encoding="utf-8"), source=str(path))


def loads(text, source="<string>"):
    """Parse scenario text; `source` names it in error messages.

    Raises `ScenarioSyntaxError` for anything outside the subset in the module docstring.
    """

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ScenarioSyntaxError(source, None, f"duplicate key {key!r}")
            result[key] = value
        return result

    def refuse_constant(name):
        # `json.loads` accepts NaN, Infinity and -Infinity by default; JSON5 does too, but this subset does not.
        raise ScenarioSyntaxError(source, None, f"{name} is not supported")

    try:
        return json.loads(_TOKEN.sub(_rewrite, text), object_pairs_hook=unique, parse_constant=refuse_constant)
    except json.JSONDecodeError as error:
        # The column is not reported: quoting keys and dropping comments shift it.
        raise ScenarioSyntaxError(source, error.lineno, error.msg) from None


def _rewrite(match):
    """Strict JSON for one `_TOKEN` match: keys gain quotes, comments and trailing commas go, the rest stays."""
    token = match.group()
    if token[0] in '"[{':
        return token
    if token.startswith("//") or token == ",":
        return ""
    return json.dumps(token)
