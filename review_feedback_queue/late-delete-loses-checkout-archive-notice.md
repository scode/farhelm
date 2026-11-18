# A slow Delete can lose the checkout archive notice

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

If Delete takes long enough to time out while the host is still working, Farhelm can remove the session without telling
you where its unarchived checkout was left.

## Details

Haiku pass H-P6-1, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed missing late outcome path in
source. The ordinary shared reply shape decodes, and the original claim of routine decode loss is unsupported.

- Verdict: NOT CRITICAL
- Route checked: `session_list/delete.rs:381–398`, `farhelm-helm/src/sessions.rs:3985–3996,3745–3805`,
  `farhelm-proto/src/lib.rs:2781–2797`, `farhelm-helm/src/ui_channel.rs:903–925,948–957`, and `SPEC.md:1895–1900`.
- Reason: The shared type makes an ordinary successful reply with a notice decodable. A slow Delete can, however, hit
  the helm's reply bound before the supervisor returns the archive outcome. The row and generic timeout can show what
  happened only to the session, and there is no later notice delivery for the folder left behind. The retained folder
  itself is untouched. The old browser reader also defaulted on decoding failure, but the ordinary host reply bound is
  part of the native migration.

Retain or publish the eventual archive outcome through the host or helm notice feed and show the path and reason even
after the session row disappears. Test a Delete that times out before the host replies that archiving failed.

Possible cover: the rare-diagnostics filter could apply if the slow-reply trigger is shown to be rare. SPEC.md:1895–1900
explicitly requires the notice, and no rarity premise was established. The existing archive and upload notice items have
different triggers.
