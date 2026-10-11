#!/usr/bin/env python3
"""Tests for scripts/todo-picker.py, the checklist page behind the "todo planning flow".

The page is only useful if the code the maintainer pastes back names exactly the entries they ticked, and if it never
offers an entry that is gone or already planned. These tests pin the parts that decide that: how TODO.md buckets and
entries are read, how a stale or incomplete summaries file is refused, and how summaries are escaped into the page.
Serving and the browser-side clipboard code are not covered; they are small and checked by opening the page.

Run with `python3 scripts/test-todo-picker.py`; stdlib only, no network.
"""

from __future__ import annotations

import importlib.util
import pathlib
import sys
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "todo-picker.py"


def load_picker():
    """Import todo-picker.py, whose hyphenated name rules out a plain import."""
    spec = importlib.util.spec_from_file_location("todo_picker", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    # dataclasses resolves string annotations through sys.modules, so the module must be registered before it runs.
    sys.modules["todo_picker"] = module
    spec.loader.exec_module(module)
    return module


picker = load_picker()

# A TODO.md shaped like the real one: an intro, a bucket with a wrapped plan reference, a bucket with subheadings and an
# entry without a bold handle, and a nested bullet that belongs to its parent.
TODO = """# TODO

Intro paragraph.

## Near term

- **Alpha thing.** Do the alpha thing, which needs `code` and
  wraps onto a second line.

- **Planned thing.** Already has a plan. Plan:
  `plans/queue/planned-thing.md`.

- **Beta thing.** Short.
  - a nested bullet that is part of beta

## Maybe later

### Small

- An entry without a handle.

### Large

- **Gamma.** Big.
"""


def filled(doc: dict) -> dict:
    """Fill a skeleton's labels and summaries the way an agent would, deriving each label from its key."""
    for entry in doc["entries"]:
        entry["label"] = entry["key"].rstrip(".")
        entry["tldr"] = "Summary of " + entry["key"]
    return doc


class ParsingTest(unittest.TestCase):
    """How buckets and entries are read; the keys tie the skeleton to the served page."""

    def test_bucket_names_match_loosely(self):
        """The maintainer names buckets from memory, so case and punctuation must not matter."""
        self.assertEqual(picker.resolve_bucket(TODO, "near-term"), "Near term")
        self.assertEqual(picker.resolve_bucket(TODO, "MAYBE LATER"), "Maybe later")
        with self.assertRaises(picker.PickerError):
            picker.resolve_bucket(TODO, "someday")

    def test_entries_keys_plans_and_groups(self):
        """Bold handles are keys, wrapped plan references are found, nested bullets stay inside their parent."""
        entries = picker.parse_bucket(TODO, "Near term")
        self.assertEqual([e.key for e in entries], ["Alpha thing.", "Planned thing.", "Beta thing."])
        self.assertEqual([e.plan for e in entries], [None, "planned-thing", None])
        self.assertIn("wraps onto a second line", entries[0].text)
        self.assertIn("nested bullet", entries[2].text)

        later = picker.parse_bucket(TODO, "Maybe later")
        self.assertEqual([(e.key, e.group) for e in later], [("An entry without a handle.", "Small"), ("Gamma.", "Large")])

    def test_skeleton_leaves_out_planned_entries(self):
        """plans/AGENTS.md never plans an entry twice, so the page must not offer one that already has a plan."""
        doc, planned = picker.skeleton(TODO, "near term")
        self.assertEqual([e["key"] for e in doc["entries"]], ["Alpha thing.", "Beta thing."])
        self.assertEqual([e.key for e in planned], ["Planned thing."])


class ValidationTest(unittest.TestCase):
    """A summaries file is served only if it still matches TODO.md exactly."""

    def test_filled_skeleton_validates(self):
        """The happy path: slugs come from labels, and groups carry through for the page's subheadings."""
        doc, _ = picker.skeleton(TODO, "Maybe later")
        bucket, items = picker.validate(TODO, filled(doc))
        self.assertEqual(bucket, "Maybe later")
        self.assertEqual([(i.slug, i.group) for i in items], [("an-entry-without-a-handle", "Small"), ("gamma", "Large")])

    def test_stale_summaries_are_refused(self):
        """An entry added to TODO.md after the skeleton must not silently go missing from the page."""
        doc, _ = picker.skeleton(TODO, "Near term")
        newer = TODO.replace("## Maybe later", "- **Delta.** New.\n\n## Maybe later")
        with self.assertRaisesRegex(picker.PickerError, "missing"):
            picker.validate(newer, filled(doc))

    def test_entry_planned_since_skeleton_is_refused(self):
        """An entry planned after the skeleton was written would otherwise be offered for planning again."""
        doc, _ = picker.skeleton(TODO, "Near term")
        planned = TODO.replace("Do the alpha thing", "Plan: `plans/queue/alpha.md`. Do the alpha thing")
        with self.assertRaisesRegex(picker.PickerError, "unexpected"):
            picker.validate(planned, filled(doc))

    def test_empty_summary_is_refused(self):
        """An unfilled entry would render as a bare checkbox the maintainer cannot judge."""
        doc, _ = picker.skeleton(TODO, "Near term")
        filled(doc)["entries"][1]["tldr"] = " "
        with self.assertRaisesRegex(picker.PickerError, "label and a tldr"):
            picker.validate(TODO, doc)

    def test_clashing_slugs_are_refused(self):
        """Two entries with the same slug would make the pasted code ambiguous."""
        doc, _ = picker.skeleton(TODO, "Near term")
        for entry in filled(doc)["entries"]:
            entry["label"] = "Same label"
        with self.assertRaisesRegex(picker.PickerError, "distinct"):
            picker.validate(TODO, doc)


class RenderingTest(unittest.TestCase):
    """Summaries are agent-written text and must not be able to break the page."""

    def test_inline_escapes_and_marks_code(self):
        """HTML is escaped, backtick spans become code, and a stray backtick stays literal."""
        self.assertEqual(picker.inline("a <b> & `x<y>`"), "a &lt;b&gt; &amp; <code>x&lt;y&gt;</code>")
        self.assertEqual(picker.inline("odd ` tick"), "odd ` tick")

    def test_page_carries_prefix_and_every_slug(self):
        """The copied code starts with the bucket's slug, and every item has a checkbox with its slug as value."""
        doc, _ = picker.skeleton(TODO, "Maybe later")
        bucket, items = picker.validate(TODO, filled(doc))
        page = picker.render_page(bucket, items)
        self.assertIn('data-prefix="todo-plan maybe-later:"', page)
        for item in items:
            self.assertIn(f'value="{item.slug}"', page)
        self.assertIn("<h2>Small</h2>", page)


if __name__ == "__main__":
    unittest.main()
