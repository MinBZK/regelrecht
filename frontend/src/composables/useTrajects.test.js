import { describe, it, expect } from 'vitest';
import { isTrajectMissing } from './useTrajects.js';

// Signature: isTrajectMissing(trajectRef, authorized, loading, errored, activeTraject)
describe('isTrajectMissing', () => {
  const ref = 'zorgtoeslag-12345678';
  const member = { ref, name: 'Zorgtoeslag' };

  it('is true when the URL names a traject not in the membership list', () => {
    // authorized, list done loading, no error, ref not found → missing
    expect(isTrajectMissing(ref, true, false, false, null)).toBe(true);
  });

  it('is false when the traject is one of the memberships', () => {
    expect(isTrajectMissing(ref, true, false, false, member)).toBe(false);
  });

  it('is false while the membership list is still loading', () => {
    expect(isTrajectMissing(ref, true, true, false, null)).toBe(false);
  });

  it('is false when the trajects fetch errored (e.g. network blip)', () => {
    // empty list from a failed fetch must not read as "does not exist"
    expect(isTrajectMissing(ref, true, false, true, null)).toBe(false);
  });

  it('is false when the user is not authorized', () => {
    expect(isTrajectMissing(ref, false, false, false, null)).toBe(false);
  });

  it('is false for the global view (no traject in the URL)', () => {
    expect(isTrajectMissing(null, true, false, false, null)).toBe(false);
  });
});
