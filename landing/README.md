# landing

Postal's landing page, privacy policy and terms: a static
[Astro](https://astro.build) site. Its one script is the install command's
copy button, hidden where scripts do not run.

```
pnpm install
pnpm dev         # http://localhost:4321
pnpm build       # into dist/
pnpm preview     # serves dist/
```

`make landing` at the repository's root lints, type-checks and builds it;
`make landing-dev` serves it.

## Lint

`pnpm lint` is Prettier's check, then ESLint; `pnpm lint:fix` and
`pnpm format` write the fixes. The rules sit under `eslint/`, a file to
a concern, each with its reason beside it: `base.js` for every file,
`astro.js` for the pages and their accessibility, `scripts.js` for tool
configs, `format.js` for what Prettier leaves to ESLint. In an `.astro`
file no rule is type-aware: an imported component has no type to read.

## Imports

`@app/*` is `src/*` and `@brand/*` is `../assets/brand/*` (`tsconfig.json`);
no relative imports.

## Brand

The site reads the brand straight from `../assets/brand/`, as `@brand/*`: `tokens.css`,
the logo, the favicons. Change the brand there (`make brand`), never here.
Pages use the semantic and component tokens (`--color-primary`,
`--space-3`, `--button-primary-bg`), not raw values.

## Domain and placeholders

- The site is served at `https://postalmail.dev`
  (`docs/adr/0028-the-site-lives-at-postalmail-dev.md`). `src/site.ts`
  holds the domain, the contact address and the date the policies take
  effect.
- The screenshots in `src/assets/media/` and the videos in
  `public/media/` come from the demo mailbox: `make stills` and
  `make media` at the root shoot them again (`demo/README.md`).

## Copy

Every claim on a page is true of Postal's code or an ADR, and uses the
words of `GLOSSARY.md`, lowercase. `docs/brand-guidelines.md` has the voice rules.
