import tseslint from 'typescript-eslint';
import emdash from './eslint-plugin-emdash/index.js';

export default tseslint.config(
  {
    ignores: ['dist/**', 'node_modules/**', 'src/bindings.ts'],
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: { emdash },
    rules: {
      // EMD-7 hard rule: every renderer cache-invalidation hop goes
      // through the `useUiMutations` bridge. See ADR-0004.
      'emdash/no-tauri-event-bus': 'error',
    },
  },
);
