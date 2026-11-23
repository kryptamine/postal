import globals from 'globals';
import astro from 'eslint-plugin-astro';
import tseslint from 'typescript-eslint';

// The pages and components: Astro's own rules and the accessibility of
// their markup, where Eris's Dashboard has React's.
export default [
  ...astro.configs['flat/recommended'],
  ...astro.configs['flat/jsx-a11y-strict'],

  {
    files: ['**/*.astro'],
    languageOptions: { globals: globals.node },
    rules: {
      // A component is its file: Astro makes the default export itself.
      'import-x/no-default-export': 'off',
      // `Astro.props` is read where the frontmatter needs it, by name.
      '@typescript-eslint/prefer-destructuring': 'off',
      // A frontmatter is a script Astro runs once to render the page.
      'unicorn/no-top-level-side-effects': 'off',
    },
  },

  {
    // An imported component has no type the checker can read, so every rule
    // that asks for one reports the whole template.
    files: ['**/*.astro'],
    extends: [tseslint.configs.disableTypeChecked],
  },
];
