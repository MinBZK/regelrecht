import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import {
  loadSavedTabs,
  saveTabs,
  loadSavedActiveTab,
  saveActiveTab,
} from './openTabsStorage.js';

// openTabsStorage keeps the editor's open law tabs per traject: each traject
// ref gets its own `regelrecht-open-tabs:<ref>` / `regelrecht-active-tab:<ref>`
// key. These pin the per-traject isolation and the try/catch safe defaults.

const TAB_A = { lawId: 'wet_op_de_zorgtoeslag', articleNumber: '1' };
const TAB_B = { lawId: 'algemene_wet_inkomensafhankelijke_regelingen', articleNumber: '7' };

beforeEach(() => {
  localStorage.clear();
});

describe('openTabsStorage', () => {
  it('persists and reloads tabs under a per-traject key', () => {
    saveTabs('traject-alpha', [TAB_A, TAB_B]);
    expect(loadSavedTabs('traject-alpha')).toEqual([TAB_A, TAB_B]);
    expect(localStorage.getItem('regelrecht-open-tabs:traject-alpha')).toBe(
      JSON.stringify([TAB_A, TAB_B]),
    );
  });

  it('isolates tabs between trajects', () => {
    saveTabs('traject-alpha', [TAB_A]);
    saveTabs('traject-beta', [TAB_B]);
    expect(loadSavedTabs('traject-alpha')).toEqual([TAB_A]);
    expect(loadSavedTabs('traject-beta')).toEqual([TAB_B]);
  });

  it('returns [] for an unknown traject', () => {
    expect(loadSavedTabs('nope')).toEqual([]);
  });

  it('returns [] when the stored value is not an array', () => {
    localStorage.setItem('regelrecht-open-tabs:x', JSON.stringify({ not: 'an array' }));
    expect(loadSavedTabs('x')).toEqual([]);
  });

  it('returns [] on malformed JSON instead of throwing', () => {
    localStorage.setItem('regelrecht-open-tabs:x', '{broken');
    expect(loadSavedTabs('x')).toEqual([]);
  });

  it('persists and reloads the active tab per traject', () => {
    saveActiveTab('traject-alpha', TAB_A);
    saveActiveTab('traject-beta', TAB_B);
    expect(loadSavedActiveTab('traject-alpha')).toEqual(TAB_A);
    expect(loadSavedActiveTab('traject-beta')).toEqual(TAB_B);
  });

  it('clears the active tab when saving a falsy value', () => {
    saveActiveTab('traject-alpha', TAB_A);
    saveActiveTab('traject-alpha', null);
    expect(loadSavedActiveTab('traject-alpha')).toBeNull();
    expect(localStorage.getItem('regelrecht-active-tab:traject-alpha')).toBeNull();
  });

  it('returns null for an unknown active tab', () => {
    expect(loadSavedActiveTab('nope')).toBeNull();
  });

  describe('safe defaults when storage is unavailable', () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it('swallows write failures (e.g. quota full / disabled)', () => {
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('QuotaExceededError');
      });
      expect(() => saveTabs('t', [TAB_A])).not.toThrow();
      expect(() => saveActiveTab('t', TAB_A)).not.toThrow();
    });

    it('returns safe defaults when reads throw', () => {
      vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
        throw new Error('SecurityError');
      });
      expect(loadSavedTabs('t')).toEqual([]);
      expect(loadSavedActiveTab('t')).toBeNull();
    });
  });
});
