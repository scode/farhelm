# Previewing drafts

This file covers the local preview server the maintainer reviews docs drafts in: how to start it, which links to hand
over, and what to do when another checkout holds its port. When this flow applies at all (main docs work only, never
unattended runs) is in `website/AGENTS.md`, "Previews and reports". It belongs to the "Docs page authoring", "Docs
refresh", "Release notes work" and "Screenshots" reading lists in `website/AGENTS.md`.

The maintainer reviews drafts in a browser, served from this checkout by `website/scripts/preview.sh` at
`http://127.0.0.1:14000/`. This local server is not a Vercel preview deployment (see `.agents/vercel.md`), and it is not
`bun run preview` either. The port is fixed so that the maintainer can forward it once; the script's header says why,
and what that costs. Use the script rather than `bun run dev` or `astro dev` directly, which pick their own port. Its
fixed port is deliberate (the script says why) and is not a harness that the fixed-port rule in `AGENTS.md`, "Sharing
the machine with other agents", applies to.

- Run `website/scripts/preview.sh` each time before you hand the maintainer a change. It starts the server if this
  checkout is not already serving, waits until it answers, and prints the docs front page URL; when the server is
  already up it only prints the URL. Running it every time rather than once per session matters because the server does
  not outlive the agent session that started it, and because its exit status is what tells you the links are good. The
  server picks up every saved edit by itself, new pages and sidebar changes included, so a browser reload shows them. A
  dependency change (`website/package.json` or `website/bun.lock`) is the exception: run
  `website/scripts/preview.sh stop`, then start it again.
- Every time you report a change under `website/`, give the maintainer a direct link to where they can see it: the URL
  of each page you changed (`website/scripts/preview.sh url <file>` prints it), with the heading anchor when the change
  sits in one section. When the change has no single page (the sidebar, styles, site configuration, a sweep across many
  pages, or a change to these instruction files), or when in doubt, link the docs front page,
  `http://127.0.0.1:14000/docs/`. The links assume the maintainer's browser reaches 127.0.0.1:14000 on this machine,
  either because it runs here or through their port forward; give them as they are.
- When another checkout's preview server holds the port, the script stops it and takes the port without asking: the
  maintainer drafts in one session at a time, so that server shows a draft nobody is reading. It never stops a
  foreground `astro dev` or anything that is not a preview server. If the script refuses because something like that
  holds the port, hand out no preview links (they would show someone else's pages, not your draft) and tell the
  maintainer what the script reported.
- Leave your own server running when you finish, since the maintainer may still be reading;
  `website/scripts/preview.sh stop` stops it when asked.

The preview does not check links. `bun run build` does (see `website/AGENTS.md`, "Internal links"), and a page that
renders fine in the preview can still fail the build.
