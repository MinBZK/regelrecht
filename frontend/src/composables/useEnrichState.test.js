import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ref } from 'vue';
import { mount } from '@vue/test-utils';

// Zelfde mock-patroon als useTasks.test.js/TasksSidebarItem.test.js: de
// netwerk-leg van useTasks.js loopt via één bestuurbare spy. useAuth komt uit
// hetzelfde gedeelde pakket, dus die hangt in dezelfde mock.
const apiFetch = vi.fn();
const authenticated = ref(true);
vi.mock('@regelrecht/frontend-shared', () => ({
  apiFetch: (...a) => apiFetch(...a),
  useAuth: () => ({ authenticated }),
}));

const push = vi.fn();
vi.mock('vue-router', () => ({ useRouter: () => ({ push: (...a) => push(...a) }) }));

const TRAJECT = 'mijn-traject-1a2b3c4d';
const LAW = 'test_wet';

function reviewTask(article, overrides = {}) {
  return {
    id: `t-${article}`,
    task_type: 'job_review',
    status: 'open',
    payload: { traject_ref: TRAJECT, law_id: LAW, article },
    ...overrides,
  };
}

function tasksResponse({ tasks = [], running = [] } = {}) {
  return { status: 200, json: async () => ({ tasks, open_count: tasks.length, running }) };
}

describe('useEnrichState', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetch.mockReset();
    push.mockReset();
    authenticated.value = true;
  });

  // Mount de composable in een echte component: de mount-refresh en
  // usePollWhile hangen aan een component-instance.
  async function mountState({ articleNumber = null, reviewActive = false, lawId = LAW } = {}) {
    const { useEnrichState } = await import('./useEnrichState.js');
    let state;
    const wrapper = mount({
      setup() {
        state = useEnrichState({
          trajectRef: ref(TRAJECT),
          lawId: ref(lawId),
          articleNumber: ref(articleNumber),
          reviewActive: ref(reviewActive),
        });
        return () => null;
      },
    });
    await wrapper.vm.$nextTick();
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    return { ...state, wrapper };
  }

  it('ververst de gedeelde takenlijst bij mount, zodat een deeplink de staat kent', async () => {
    apiFetch.mockResolvedValue(tasksResponse({ tasks: [reviewTask('2')] }));
    const { reviewReady, reviewArticleForPane } = await mountState({ articleNumber: '2' });
    expect(apiFetch).toHaveBeenCalledWith('/api/tasks', expect.anything());
    expect(reviewReady.value).toBe(true);
    expect(reviewArticleForPane.value).toBe('2');
  });

  it('haalt de takenlijst niet op voor een anonieme bezoeker', async () => {
    authenticated.value = false;
    apiFetch.mockResolvedValue(tasksResponse());
    await mountState();
    expect(apiFetch).not.toHaveBeenCalled();
  });

  it('kiest de taak van het geopende artikel, anders de eerste van de wet', async () => {
    apiFetch.mockResolvedValue(
      tasksResponse({ tasks: [reviewTask('1'), reviewTask('3')] }),
    );
    const here = await mountState({ articleNumber: '3' });
    expect(here.pendingReviewTask.value.id).toBe('t-3');

    vi.resetModules();
    const elsewhere = await mountState({ articleNumber: '7' });
    expect(elsewhere.pendingReviewTask.value.id).toBe('t-1');
  });

  it('meldt geen klaarstaand voorstel terwijl je die taak al beoordeelt', async () => {
    apiFetch.mockResolvedValue(tasksResponse({ tasks: [reviewTask('2')] }));
    const { pendingReviewTask, reviewReady } = await mountState({
      articleNumber: '2',
      reviewActive: true,
    });
    expect(pendingReviewTask.value).not.toBeNull();
    expect(reviewReady.value).toBe(false);
  });

  it('negeert taken van een andere wet', async () => {
    apiFetch.mockResolvedValue(
      tasksResponse({ tasks: [reviewTask('2', { payload: { law_id: 'andere_wet' } })] }),
    );
    const { reviewReady } = await mountState();
    expect(reviewReady.value).toBe(false);
  });

  it('staat in de bezig-staat zolang er een enrich-job voor deze wet loopt', async () => {
    apiFetch.mockResolvedValue(
      tasksResponse({ running: [{ job_id: 'j1', job_type: 'enrich', law_id: LAW }] }),
    );
    const { isEnriching } = await mountState();
    expect(isEnriching.value).toBe(true);
  });

  // De bug die deze composable opheft: zonder refresh na de aanvraag blijft
  // isEnriching false en begint de gerichte poll nooit.
  it('ververst de takenlijst na een verrijk-aanvraag, zodat de bezig-staat omslaat', async () => {
    apiFetch.mockResolvedValue(tasksResponse());
    const { isEnriching, requestEnrich } = await mountState();
    expect(isEnriching.value).toBe(false);

    apiFetch.mockReset();
    apiFetch.mockImplementation(async (url) =>
      url.endsWith('/enrich')
        ? { status: 200 }
        : tasksResponse({ running: [{ job_id: 'j1', job_type: 'enrich', law_id: LAW }] }),
    );
    const result = await requestEnrich();
    expect(result).toEqual({ alreadyRunning: false, tooMany: false });
    expect(apiFetch).toHaveBeenCalledWith(
      `/api/trajects/${TRAJECT}/corpus/laws/${LAW}/enrich`,
      expect.objectContaining({ method: 'POST' }),
    );
    expect(apiFetch).toHaveBeenCalledWith('/api/tasks', expect.anything());
    expect(isEnriching.value).toBe(true);
  });

  it('opent de takenlijst gefilterd op deze wet', async () => {
    apiFetch.mockResolvedValue(tasksResponse());
    const { openTasksForLaw } = await mountState();
    openTasksForLaw();
    expect(push).toHaveBeenCalledWith({
      name: 'taken-traject',
      params: { trajectRef: TRAJECT, categorie: 'wet', contextLawId: LAW },
    });
  });

  it('doet niets zonder wet of traject', async () => {
    apiFetch.mockResolvedValue(tasksResponse());
    const { openTasksForLaw, openReviewForLaw } = await mountState({ lawId: null });
    openTasksForLaw();
    openReviewForLaw();
    expect(push).not.toHaveBeenCalled();
  });

  it('navigeert naar de beoordeling van het klaarstaande voorstel', async () => {
    apiFetch.mockResolvedValue(tasksResponse({ tasks: [reviewTask('2')] }));
    const { openReviewForLaw } = await mountState({ articleNumber: '2' });
    openReviewForLaw();
    expect(push).toHaveBeenCalledWith(
      expect.objectContaining({ params: expect.objectContaining({ lawId: LAW }) }),
    );
  });
});
