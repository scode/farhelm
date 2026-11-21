# Failed Suspend can leave the next pause credited to an earlier one

Reviewed commit: 405b3a3dcec6216ed9252d7a5128de649eab8530

## TLDR

A later outside pause can be credited to an old Suspend after an explicit host operation already saw the sandbox
running.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed by source, without runtime observation. A saved stamp can survive a restart while an initial actor probe is
still pending or fails to get provider status. Suspend parks the actor before reading Tensorlake
(environment/lifecycle.rs:92–102). The lifecycle operation reads Running (tensorlake/lifecycle.rs:197–205) but discards
that fact if the requested mutation fails or the operation times out (lines 207–249). The error short-circuits the
timestamp callback and releases the hold (environment/lifecycle.rs:101–125). If the sandbox is paused outside Farhelm
before the actor's next answer, the actor still has no newer Running evidence and accepts the old stamp
(manager.rs:4197–4198,4365–4388,4852–4877). The same omission can retain the stamp after a successful Running reply in
explicit Resume followed by an error writing the clear. SPEC.md:340–347 promises pause attribution and
SPEC_impl.md:6723–6727 says a Running answer retires older attribution even if the write fails. The stated exceptions at
SPEC_impl.md:6777–6781 cover an unseen resume and re-pause, or a failed clear followed by a restart and then an outside
pause. In this route the Running answer reached the helm and attribution goes stale while it remains open. Estimated
bucket other, wording only. Possible_cover: the retirement requirement and nearby limitations above. The assigned
follow-up 84bf819d21f41e8a8b808b8cf90aa1ccc44981a1 only changed field comments and did not address the route. The
relevant lifecycle code remains unchanged at 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. The Hosts action supports this
route while the host is unreachable and the dialog submits the same host it displayed (GUI hosts/offers.rs:302–303,
hosts/lifecycle.rs:177–220,249–263). The existing failure test starts with the actor already connected and waits for
reconnection after refusal (tensorlake/hosts_tests.rs:1966–1992), so it does not isolate lifecycle-only Running
evidence. The session card makes the mislabeled attribution visible (environment_ui.rs:728–734). Synthesis: An explicit
Resume that sees Running clears the database synchronously under the provisioning lock, but it never advances the
actor's Running time, including when the sandbox was already running and no reconnect is forced
(tensorlake/lifecycle.rs:114–121,148–178,197–219. Environment/lifecycle.rs:92–125). The success path can also show stale
wording in the same app even if the clear commits: a disconnected Resume parks after reading the old stamp, the sandbox
is paused again before the first unheld actor probe, and that pass's registry read fails. The actor uses its cached
older row (manager.rs:4137–4163,5866–5884), then accepts the stamp without newer actor evidence (4365–4388). A later
successful registry read of the cleared row does not remove the stamp from a still-Suspended snapshot because the
snapshot is preferred. This strengthens the scope from write failure to a separate stale-read condition under the same
missing lifecycle evidence. A successful Resume with fresh registry data is outside the failure route.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `tensorlake/f4`.
