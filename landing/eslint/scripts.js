import globals from 'globals';
import tseslint from 'typescript-eslint';

// What is no source of the site: a tool's config.
export default [
  {
    // A config is the call its tool asks for, `export default defineConfig(…)`,
    // and that call is the side effect the rule names.
    files: ['**/*.config.{ts,js,mjs}', 'eslint/*.js'],
    languageOptions: { globals: globals.node },
    rules: {
      'import-x/no-default-export': 'off',
      'unicorn/no-top-level-side-effects': 'off',
    },
  },

  {
    // No config is in the tsconfig, so no rule on one can be type-aware.
    files: ['**/*.{js,mjs}'],
    extends: [tseslint.configs.disableTypeChecked],
    rules: {
      'import-x/no-named-as-default': 'off',
      'import-x/no-named-as-default-member': 'off',
      // The file carries no types, so a return type has nothing to say.
      '@typescript-eslint/explicit-function-return-type': 'off',
    },
  },
];
