// @ts-check
import { defineConfig } from 'astro/config';

import { site } from './src/site.ts';

export default defineConfig({
  site: site.url,
  vite: {
    // The brand files live one directory up, in assets/brand.
    server: { fs: { allow: ['..'] } },
    // An icon inlined as a data URI is not one every browser takes.
    build: { assetsInlineLimit: 0 },
  },
});
