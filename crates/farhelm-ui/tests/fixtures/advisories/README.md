# Advisory authentication fixtures

These files use a throwaway Ed25519 test key, absent from the production release ring. The secret key was generated
nonextractably in a short-lived Deno WebCrypto process and never serialized; only its public half and signatures remain.
The process signed the Blake2b-512 digest, and the signature plus trusted comment, in minisign's wire format. No test
requires a secret or invokes a signer. `legacy.minisig` signs the raw body to exercise the non-prehashed refusal.

`future` is a permanent compatibility fixture: it includes unknown directives a later release might publish, alongside
known withdrawals. Old clients must authenticate the whole file, ignore the unknown lines and retain the withdrawals.
`wrong-purpose.minisig` authenticates those same bytes as `farhelm v1.2.3`, proving that release signatures cannot be
replayed as advisories. `empty` is a valid signed snapshot lifting all withdrawals, not a missing or invalid file.
