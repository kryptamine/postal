import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import regexp from 'eslint-plugin-regexp';
import promise from 'eslint-plugin-promise';
import sonarjs from 'eslint-plugin-sonarjs';
import unicorn from 'eslint-plugin-unicorn';
import importX from 'eslint-plugin-import-x';
import perfectionist from 'eslint-plugin-perfectionist';

import { landingRoot, namedUnionMembers, upADirectory } from './shared.js';

// What every file is held to. The rules and their reasons are Eris's, less
// what names its own layers.
export default [
  {
    files: ['**/*.{ts,js,mjs,astro}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
      importX.flatConfigs.recommended,
      importX.flatConfigs.typescript,
      unicorn.configs.recommended,
      sonarjs.configs.recommended,
      promise.configs['flat/recommended'],
      regexp.configs['flat/recommended'],
    ],
    plugins: { perfectionist },
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: landingRoot,
      },
    },
    settings: {
      'import-x/resolver': {
        typescript: { project: [`${landingRoot}/tsconfig.json`] },
      },
    },
    rules: {
      // Cyclomatic complexity is not measured beside it: `?.` and `??` each
      // count as a branch, so it scores a flat block of defaults like a thicket.
      'sonarjs/cognitive-complexity': ['error', 12],
      // @typescript-eslint/no-deprecated reports the same, at a fifth of the cost.
      'sonarjs/deprecation': 'off',
      // The aws-* rules check AWS CDK constructs, which nothing here uses.
      ...Object.fromEntries(
        Object.keys(sonarjs.rules)
          .filter((name) => name.startsWith('aws-'))
          .map((name) => [`sonarjs/${name}`, 'off']),
      ),
      // A type says what a value is. An interface additionally declares itself
      // open to merging, which is a promise nothing here wants to make.
      '@typescript-eslint/consistent-type-definitions': ['error', 'type'],
      // A case added to a union fails every switch over it, which a default
      // would swallow.
      '@typescript-eslint/switch-exhaustiveness-check': [
        'error',
        { allowDefaultCaseForExhaustiveSwitch: false, considerDefaultExhaustiveForUnions: false },
      ],
      '@typescript-eslint/consistent-type-imports': ['error', { fixStyle: 'inline-type-imports' }],
      '@typescript-eslint/explicit-function-return-type': [
        'error',
        { allowExpressions: true, allowTypedFunctionExpressions: true },
      ],
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      // Node's own, then packages, then our aliases, then what sits beside the
      // file. Inside a group the shorter import comes first, so one of a single
      // name never sits below one of many.
      'import-x/order': 'off',
      'perfectionist/sort-imports': [
        'error',
        {
          type: 'line-length',
          order: 'asc',
          fallbackSort: { type: 'alphabetical', order: 'asc' },
          internalPattern: ['^@(app|brand)/'],
          groups: [
            'builtin',
            'external',
            'internal',
            ['parent', 'sibling', 'index'],
            // Never moved, because a side effect's place can matter.
            ['side-effect-style', 'side-effect'],
            'unknown',
          ],
          newlinesBetween: 1,
        },
      ],
      'import-x/no-default-export': 'error',
      // Vite answers an asset query (`?raw`, `?url`) and Astro its own
      // `astro:` modules; the resolver knows neither.
      'import-x/no-unresolved': ['error', { ignore: [String.raw`\?(raw|url)$`, '^astro:'] }],
      // TypeScript answers these three, and they walk every imported module
      // again to do it.
      'import-x/default': 'off',
      'import-x/namespace': 'off',
      'import-x/no-named-as-default-member': 'off',
      '@typescript-eslint/no-restricted-imports': ['error', { patterns: [upADirectory] }],
      'no-restricted-syntax': ['error', namedUnionMembers],
      // Return early: what follows a return needs no else around it.
      'no-else-return': ['error', { allowElseIf: false }],
      // Arrow functions everywhere. allowUnboundThis leaves the one case that
      // needs the keyword: a callback that is called with a `this`.
      'func-style': ['error', 'expression', { allowArrowFunctions: true }],
      'prefer-arrow-callback': ['error', { allowNamedFunctions: false, allowUnboundThis: true }],
      // A declaration takes a field by its own name through destructuring.
      // Assignments and renames stay as written.
      'prefer-destructuring': 'off',
      '@typescript-eslint/prefer-destructuring': [
        'error',
        {
          VariableDeclarator: { object: true, array: true },
          AssignmentExpression: { object: false, array: false },
        },
      ],
      // Names here come from GLOSSARY.md and from Astro, `props` among them,
      // and the replacement lists know neither.
      'unicorn/name-replacements': 'off',
      'unicorn/prevent-abbreviations': 'off',
      'unicorn/filename-case': ['error', { cases: { kebabCase: true, pascalCase: true } }],
      // A ternary is kept for the value it tested; this folds every early
      // return into one.
      'unicorn/prefer-ternary': 'off',
      // The rule reads no types, so it asks for a default where null is held.
      'unicorn/prefer-default-parameters': 'off',
      // This moves every named type into the one function that reads it.
      'unicorn/prefer-smaller-scope': 'off',
      // A predicate is named for what it asks, and the rule wants an `is`
      // before it.
      'unicorn/consistent-boolean-name': 'off',
      // Each early return carries the comment of its own reason, which one
      // guard joined by `||` has no place for.
      'unicorn/prefer-combined-guards': 'off',
    },
  },

  {
    // A constant a module declares at its top — a string, a number, a flag —
    // is written as one: POLICY_DATE, not policyDate. Functions and objects
    // keep their own case.
    files: ['**/*.ts'],
    rules: {
      '@typescript-eslint/naming-convention': [
        'error',
        {
          selector: 'variable',
          modifiers: ['const', 'global'],
          types: ['boolean', 'string', 'number'],
          format: ['UPPER_CASE'],
        },
      ],
    },
  },
];
