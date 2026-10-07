"""Pin old-release trust selection and exact version binding before installer execution."""

import unittest

from verify_release import key_ring, require_comment


class VerificationTests(unittest.TestCase):
    """Reject plausible-looking verification inputs that do not establish the contract."""

    def test_only_compiled_ring_is_used(self):
        """Commented key references must not broaden what the previous release trusts."""
        source = '// RELEASE_KEY_RING mentions "nottrusted"\npub const RELEASE_KEY_RING: &[&str] = &["keyA", "keyB"];'
        self.assertEqual(key_ring(source), ['keyA', 'keyB'])

    def test_missing_ring_is_not_replaced_by_current_keys(self):
        """Unknown old source must fail closed rather than substituting another release."""
        with self.assertRaises(ValueError):
            key_ring('// no compiled ring')

    def test_full_commented_declarations_cannot_supply_trust(self):
        """A retired declaration must not replace the ring the old compiler used."""
        retired = 'pub const RELEASE_KEY_RING: &[&str] = &["retiredKey"];'
        active = 'pub const RELEASE_KEY_RING: &[&str] = &["compiledKey"];'
        for comment in ('// ' + retired + '\n', '/* ' + retired + ' */',
                        '/* nested /* old */ ' + retired + ' */'):
            with self.subTest(comment=comment):
                self.assertEqual(key_ring(comment + '\n' + active), ['compiledKey'])
                with self.assertRaises(ValueError):
                    key_ring(comment)

    def test_quoted_examples_and_ambiguous_declarations_are_refused(self):
        """String examples are not code; multiple active rings need explicit inspection."""
        active = 'pub const RELEASE_KEY_RING: &[&str] = &["key//A", "keyB"];'
        for example in ('const EXAMPLE: &str = r#"pub const RELEASE_KEY_RING: &[&str] = &["retired"];"#;',
                        'const EXAMPLE: &str = "pub const RELEASE_KEY_RING: &[&str] = &[\\"retired\\"];";',
                        "const SLASH: char = '/'; // historical note\n"):
            with self.subTest(example=example):
                self.assertEqual(key_ring(example + '\n' + active), ['key//A', 'keyB'])
        with self.assertRaises(ValueError):
            key_ring(active + '\n' + active)

    def test_commented_array_entries_cannot_expand_trust(self):
        """A quoted disabled key inside the array must never verify a release.

String literals can contain slash characters, so removing comments blindly
would also corrupt real base64 keys. Tokenize literals and comments separately.
"""
        source = 'pub const RELEASE_KEY_RING: &[&str] = &["key//A", // "disabledB"\n /* "disabledC" */ "keyD"];'
        self.assertEqual(key_ring(source), ['key//A', 'keyD'])

    def test_unrecognized_rust_array_expressions_are_refused(self):
        """A source-layout change needs inspection, not inferred trust from quoted text."""
        for expression in ('concat!("keyA", "keyB")', '/* nested /* comment */ "keyC" */ "keyD"'):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                key_ring(f'pub const RELEASE_KEY_RING: &[&str] = &[{expression}];')

    def test_trusted_comment_requires_exact_tag_once(self):
        """Wrong versions, a doubled v and duplicate comments must all be refused."""
        require_comment('trusted comment: farhelm v0.24.0', 'v0.24.0')
        for invalid in ('trusted comment: farhelm vv0.24.0',
                        'trusted comment: farhelm v0.23.0',
                        'trusted comment: farhelm v0.24.0\ntrusted comment: farhelm v0.24.0'):
            with self.subTest(signature=invalid), self.assertRaises(ValueError):
                require_comment(invalid, 'v0.24.0')


if __name__ == '__main__':
    unittest.main()
