import starlight from '@astrojs/starlight';
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
