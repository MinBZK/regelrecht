import { describe, it, expect, vi } from 'vitest';
import { ref } from 'vue';
import { useResolvedDraftNotes } from './useNotes.js';

// Capture what useResolvedDraftNotes hands the WASM engine. The mock replaces
// the singleton engine bootstrap.
const engineCalls = vi.hoisted(() => []);
vi.mock('./useEngine.js', () => ({
  useEngine: () => ({
    initEngine: async () => ({
      resolveNote(lawId, _selector, validFrom) {
        engineCalls.push({ lawId, validFrom });
        return {
          status: 'found',
          matches: [{ article_number: '1', start: 0, end: 3 }],
        };
      },
    }),
    loadDependency: async () => {},
  }),
}));

describe('useResolvedDraftNotes', () => {
  it('resolves drafts against the viewed law version, not the newest', async () => {
    // The engine holds every version of the law; the composable must pass
    // the viewed version's valid_from through so a draft highlights in the
    // text on screen instead of the newest loaded version.
    engineCalls.length = 0;
    const drafts = ref([{ target: { selector: { exact: 'abc' } } }]);
    const lawId = ref('wet_x');
    const selectedArticle = ref({ number: '1' });
    const trajectRef = ref(null);
    const validFrom = ref('2024-01-01');
    const { draftNotesForArticle } = useResolvedDraftNotes(
      drafts,
      lawId,
      selectedArticle,
      trajectRef,
      validFrom,
    );
    await vi.waitFor(() => expect(engineCalls).toHaveLength(1));
    expect(engineCalls[0]).toEqual({ lawId: 'wet_x', validFrom: '2024-01-01' });
    expect(draftNotesForArticle.value).toHaveLength(1);
  });

  it('passes undefined (= latest) when the law has no valid_from', async () => {
    engineCalls.length = 0;
    const drafts = ref([{ target: { selector: { exact: 'abc' } } }]);
    useResolvedDraftNotes(drafts, ref('wet_x'), ref({ number: '1' }), ref(null), ref(null));
    await vi.waitFor(() => expect(engineCalls).toHaveLength(1));
    expect(engineCalls[0].validFrom).toBeUndefined();
  });
});
