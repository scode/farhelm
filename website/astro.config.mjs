import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator';
import { defineConfig } from 'astro/config';

// The published site is one Astro project that will eventually carry the
// farhelm.io landing pages as well as the documentation. Starlight owns the
// routes of its content collection, so the docs live one directory deeper
// than Starlight's default (`src/content/docs/docs/`) and render under
// `/docs/`; the root redirect stands in for the landing page until one exists.
// Sidebar slugs are collection slugs, so they carry that `docs/` prefix too.
export default defineConfig({
  site: 'https://farhelm.io',
  redirects: {
    '/': '/docs/',
  },
  vite: {
    build: {
      rolldownOptions: {
        // Astro marks the module it generates for each MDX page with a
        // "use astro:head-inject" directive. Vite 8's bundler (Rolldown)
        // does not know that directive, so it warns once per MDX page that
        // it may be dropped. Nothing reads the directive after bundling (the
        // head assets are chosen by module id instead), so the warning is
        // noise, and it grows with every MDX page added. Only that exact
        // combination is dropped: any other MODULE_LEVEL_DIRECTIVE warning,
        // say a misplaced "use client", still reaches the log. Remove this
        // once Astro stops emitting the directive or Rolldown stops warning
        // about it.
        onLog(level, log, handler) {
          if (log.code === 'MODULE_LEVEL_DIRECTIVE' && log.message?.includes('use astro:head-inject')) {
            return;
          }
          handler(level, log);
        },
      },
    },
  },
  integrations: [
    starlight({
      title: 'Farhelm',
      description: 'Farhelm documentation.',
      // The header mark is the README's: icon plus wordmark, rendered by
      // scripts/render-svgs.mjs, one file per color scheme.
      logo: {
        light: './src/site-mark-light.svg',
        dark: './src/site-mark-dark.svg',
        replacesTitle: true,
        alt: 'farhelm',
      },
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
      },
      customCss: ['./src/styles/farhelm.css'],
      // Every internal link, heading anchors included, is checked at build
      // time, so a moved page or a renamed heading fails `bun run build`
      // (and with it CI and the Vercel deploy) instead of shipping a dead
      // link. The pages cross-reference each other heavily by design (see
      // AGENTS.md next to this file), which is what makes this worth a
      // hard failure. The defaults already reject relative links, which
      // the pages do not use; the one change is refusing links written as
      // full https://farhelm.io URLs, which the plugin would otherwise skip
      // unchecked, so every internal link stays a checked site path.
      plugins: [starlightLinksValidator({ sameSitePolicy: 'error' })],
      // The sidebar is the documentation's outline, in reading order: a
      // guided first run, task guides, agents, then the mental model.
      // Groups autogenerate from their directories and pages order
      // themselves with `sidebar.order` frontmatter, so adding a page never
      // means editing this list. Pages not yet written carry a "Stub" badge
      // (see AGENTS.md next to this file).
      sidebar: [
        { label: 'Overview', slug: 'docs' },
        { label: 'Get started', items: [{ autogenerate: { directory: 'docs/get-started' } }] },
        { label: 'Using Farhelm', items: [{ autogenerate: { directory: 'docs/using' } }] },
        { label: 'Agents', items: [{ autogenerate: { directory: 'docs/agents' } }] },
        { label: 'How it works', items: [{ autogenerate: { directory: 'docs/how-it-works' } }] },
      ],
    }),
  ],
});
