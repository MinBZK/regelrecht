import { describe, it, expect } from 'vitest';
import { isTrajectMissing } from './useTrajects.js';

describe('isTrajectMissing', () => {
  const ref = 'zorgtoeslag-12345678';
  const member = { ref, name: 'Zorgtoeslag' };

  it('is true when the URL names a traject not in the membership list', () => {
    // authenticated, list done loading, ref not found → missing
    expect(isTrajectMissing(ref, true, false, null)).toBe(true);
  });

  it('is false when the traject is one of the memberships', () => {
    expect(isTrajectMissing(ref, true, false, member)).toBe(false);
  });

  it('is false while the membership list is still loading', () => {
    expect(isTrajectMissing(ref, true, true, null)).toBe(false);
  });

  it('is false when the user is not authenticated', () => {
    expect(isTrajectMissing(ref, false, false, null)).toBe(false);
  });

  it('is false for the global view (no traject in the URL)', () => {
    expect(isTrajectMissing(null, true, false, null)).toBe(false);
  });
});
