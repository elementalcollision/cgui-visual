// Global vitest setup. Imported by every test via vitest.config.ts.
import '@testing-library/jest-dom/vitest';

// vite.config.ts inlines `__APP_VERSION__` via the `define` option so
// production code can render the live version without a backend
// round-trip. vitest doesn't apply `define` substitutions to test
// modules, so any component that reads the global crashes with
// "__APP_VERSION__ is not defined". Mirror the substitution here so
// the Sidebar (and any future consumer) renders cleanly under test.
// Value doesn't need to match release reality — tests assert on
// behaviour, not on the literal version string.
// @ts-expect-error injected for tests; declared in vite-env.d.ts for prod.
globalThis.__APP_VERSION__ = '0.0.0-test';
