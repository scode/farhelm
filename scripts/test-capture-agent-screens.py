#!/usr/bin/env python3
"""Keep configured local proxy credentials out of public screen fixtures without running a vendor agent."""

import importlib.util
from pathlib import Path
import sys
import unittest

_spec = importlib.util.spec_from_file_location(
    "capture_agent_screens", Path(__file__).with_name("capture-agent-screens.py")
)
capture = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = capture
_spec.loader.exec_module(capture)


class ScrubberTests(unittest.TestCase):
    """A local URL is private even when userinfo precedes its host; public links still belong in a fixture."""

    def test_local_urls_with_userinfo_are_replaced_and_refused(self):
        """The replacement removes credentials, and the backstop refuses each unsanitized authority independently."""
        scrubber = capture.Scrubber()
        for host in (
            "localhost",
            "[::1]",
            "[::]",
            "[fd00::1]",
            "[fe80::1%25eth0]",
            "[::ffff:192.168.1.4]",
            "127.0.0.1",
            "10.0.0.2",
        ):
            for userinfo in ("", "user@", "user:pass@", "user:p@ss@", "user:p,ass@"):
                for suffix in ("/backend-api/codex", "?mode=proxy", "#proxy"):
                    with self.subTest(host=host, userinfo=userinfo, suffix=suffix):
                        text = f"base URL: http://{userinfo}{host}:8080{suffix}"
                        self.assertIsNotNone(scrubber.check(text))
                        clean = scrubber.scrub(text)
                        self.assertEqual(clean, "base URL: http://proxy.invalid" + suffix)
                        self.assertIsNone(scrubber.check(clean))

    def test_public_hosts_are_not_changed_by_at_signs_outside_the_authority(self):
        """A path or query naming a local host must not make an ordinary public documentation link private."""
        scrubber = capture.Scrubber()
        for text in (
            "https://docs.example.com/user@localhost",
            "https://docs.example.com/?next=user@localhost",
            "https://[2001:db8::1]/user@localhost",
            "https://localhost.example.com/help",
            "https://localhost:8080,user@example.com/guide",
            "https://localhost:8080,user@example.com?view=guide",
        ):
            with self.subTest(text=text):
                self.assertEqual(scrubber.scrub(text), text)
                self.assertIsNone(scrubber.check(text))

    def test_comma_can_separate_a_local_url_from_prose(self):
        """The adjacent address stays readable even though the proxy authority has to go."""
        scrubber = capture.Scrubber()
        for suffix in ("", "?", "#"):
            text = "http://localhost:8080,user@example.com" + suffix
            self.assertIsNotNone(scrubber.check(text))
            self.assertEqual(scrubber.scrub(text), "http://proxy.invalid,user@example.com" + suffix)

    def test_quoted_local_authority_is_redacted_inside_the_quotes(self):
        """Configuration syntax can surround a local URL with quotes and terminate it before any path."""
        scrubber = capture.Scrubber()
        for host in ("127.0.0.1.", "[fd00::1]"):
            text = f'"http://user:p@ss@{host}:8080"'
            self.assertIsNotNone(scrubber.check(text))
            self.assertEqual(scrubber.scrub(text), '"http://proxy.invalid"')

    def test_complete_private_authority_wins_over_punctuation_prefix(self):
        """A privacy scrub removes all of the credentials even when their prefix resembles an adjacent URL."""
        scrubber = capture.Scrubber()
        for suffix in ("/guide", ",user@example.com"):
            text = "http://localhost:123,p@ss@localhost:8080" + suffix
            self.assertIsNotNone(scrubber.check(text))
            clean = scrubber.scrub(text)
            self.assertEqual(clean, "http://proxy.invalid" + suffix)
            self.assertIsNone(scrubber.check(clean))


if __name__ == "__main__":
    unittest.main()
