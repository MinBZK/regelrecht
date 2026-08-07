import { describe, it, expect } from 'vitest';

import * as shared from '@regelrecht/frontend-shared';

import * as reexport from './apiFetch.js';

// The behaviour of apiFetch is proven once, in
// packages/frontend-shared/src/apiFetch.test.js. The editor owns no
// implementation, only the re-export that keeps ~25 call sites importing from
// './lib/apiFetch.js'. What can break here is the re-export losing a name.
describe('lib/apiFetch re-export', () => {
  it.each(['apiFetch', 'apiFetchJson', 'apiFetchText', 'ApiError'])(
    're-exports %s from the shared package',
    (name) => {
      expect(reexport[name]).toBe(shared[name]);
    },
  );
});
