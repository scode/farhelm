# Both typed search layouts can spend quadratic UI-thread work underlining a bounded saved wrapper

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Underlining matches in a long saved wrapper can do enough work on the GUI thread to slow launcher search.

## Details

**high.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/host_text.rs:515–551,555–565;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/view/search.rs:908–950,1072–1079;
crates/gui/farhelm-gui/src/launcher/view/search/start.rs:467–477,514–519,555–565;
crates/farhelm-supervisor/src/service/handlers.rs:86,381–411; crates/farhelm-proto/src/session_launch.rs:546–552;
crates/farhelm-helm/src/launches.rs:537–544,694–705,1039–1047

For H ordered, nonoverlapping ASCII hits, the interval traversal alone performs at least H(H+1)/2 membership checks for
the intervals that begin on the hits. Both layouts pass the full joined saved wrapper through HostSentence before width
clipping. A recent row can be admitted by a match on destination, harness or model independently of wrapper text
(launcher/search.rs:700–719). This concerns user-originated selections actually stored after create (helm
sessions.rs:2162–2170,2290–2334; store.rs:202–245,9693–9732). The supervisor's 64 KiB aggregate create cap charges JSON
for the stored selection and the compiled start argv, plus the resume argv where present. The raw joined wrapper alone
therefore cannot exceed 32 KiB, or about 21 KiB if serialized in all three places, even before JSON overhead and the
other launch fields. The wrapper validator adds shape checks without a smaller byte cap. This still permits lengths on
the order of thousands of characters, so the quadratic work bound remains material. These are admission bounds, not
measured lengths or latencies. The ordinary history window caps at 100 per host/install and the separate environment
window caps at 100 (helm store.rs:88–91,9516–9536; store/environment_history.rs:93–116); typed recents can merge both
without the empty field's three-row display cap (launcher/recents.rs:143–237; launcher/search.rs:700–719). The Host
picker is outside this route: its typeahead compares one prefix per choice, keeps only what fits the longest name, and
draws plain escaped HostSentence values (host_picker.rs:107–112,247–291,497–502,623–639). Host aliases are capped at 64
characters (helm store.rs:2154–2172). Keep high based on the reachable render work on accepted stored data, with stall
duration and prevalence unverified. Merge hit ranges before combining them with font boundaries and advance through them
once.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-hosts/launcher-hosts-quadratic-underlining`.
