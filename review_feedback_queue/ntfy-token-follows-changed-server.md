# Changing the notification server forwards its saved token

Reviewed commit: 786800be79d2a85c7921ee2aa3d7c46c4887098d

## TLDR

Changing the notification server without replacing or clearing its token sends the old server's access token to the new
server.

## Details

Suggested bucket: `highest`. Confidence: confirmed static mechanism; no network reproduction. Source: dedicated
`gpt-6.1-sol` high adversarial review, followed by a fresh independent `gpt-6.1-sol` high drop audit. Proposed drop: the
original reviewer proposed accepted-behavior coverage; the independent reviewer rejected that basis for the cross-origin
consequence. Keep the finding for triage.

`crates/farhelm-helm/src/ntfy.rs:63–81` retains `previous.token` when the submitted token is omitted/null, even if the
server URL changes. `363–371` sends the retained token as bearer authentication to the resulting server.
`crates/farhelm-ui/src/ntfy_settings.rs:14–26` omits the token when the replacement field is blank.

Save a token for server A, change the URL to server B, leave the replacement token blank, then send a test alert or
receive a background alert. B receives A's bearer token. A stale second GUI can also submit a new server without knowing
that another GUI added a token meanwhile; this is the same credential consequence, not a separate finding.

Possible cover: SPEC_impl.md, Ntfy waiting alerts, reviewed lines `3119–3127` (current `3161–3169`), explicitly
preserves omitted/null tokens. SPEC.md, Security, reviewed `2126–2129`, describes sending the optional token to the
saved server. Neither separately discusses forwarding a credential across origins. The stale-GUI variant also intersects
SPEC.md's One GUI at a time acceptance (`2506–2519` at the reviewed commit). The independent drop audit found that
general preservation and multi-GUI race acceptance do not explicitly settle this credential-disclosure consequence.

Current-main status: persists unchanged at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. An
origin-change rule should be settled for retained tokens, including the server's atomic save path. A regression can use
two controlled loopback servers and verify that changing origin cannot send the original bearer token without an
explicit decision to reuse it.
