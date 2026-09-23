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
      // docs/readme/render-svgs.mjs, one file per color scheme.
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
      sidebar: [
        { label: 'Overview', slug: 'docs' },
      ],
    }),
  ],
});
