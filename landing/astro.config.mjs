// @ts-check
import { defineConfig } from 'astro/config';

import { site } from './src/site.ts';

export default defineConfig({
  site: site.url,
  // Compression drops a line break before an inline tag, gluing a link to the word before it.
  compressHTML: false,
  vite: {
    // The brand files live one directory up, in assets/brand.
    server: { fs: { allow: ['..'] } },
    // An icon inlined as a data URI is not one every browser takes.
    build: { assetsInlineLimit: 0 },
  },
});
