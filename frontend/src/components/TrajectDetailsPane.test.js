import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import TrajectDetailsPane from './TrajectDetailsPane.vue';

// De pane praat via twee poten met de backend: het detail-GET (apiFetchJson, in
// useTrajectDetail) en de PATCH (apiFetch, in useTrajects). Beide mocken we hier
// en verder draaien de echte composables mee.
const apiFetch = vi.fn();
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch.js', () => ({
  apiFetch: (...a) => apiFetch(...a),
  apiFetchJson: (...a) => apiFetchJson(...a),
}));

// Rollen komen uit de sessie; standaard heeft de gebruiker er geen.
const roles = new Set();
vi.mock('../composables/useAuth.js', () => ({
  hasRole: (role) => roles.has(role),
}));

vi.mock('vue-router', () => ({
  useRoute: () => ({ params: {} }),
  useRouter: () => ({ push: vi.fn() }),
}));

const TRAJECT_ID = '11111111-2222-3333-4444-555555555555';

// Fictieve repo-coördinaten: dit is een publieke repo.
function ownSource(overrides = {}) {
  return {
    source_id: 'traject-own',
    name: 'Eigen repo',
    source_type: 'github',
    gh_owner: 'example-org',
    gh_repo: 'regelrecht-corpus-example',
    gh_branch: 'traject/voorbeeld',
    gh_base_branch: 'main',
    gh_path: null,
    priority: 0,
    auth_ref: 'example-org-regelrecht-corpus-example',
    is_writable_own: true,
    ...overrides,
  };
}

function detail({ role = 'owner', source = ownSource() } = {}) {
  return {
    id: TRAJECT_ID,
    name: 'Voorbeeldtraject',
    description: '',
    status: 'bezig',
    role,
    ref: 'voorbeeldtraject-abcd1234',
    members: [],
    pending_invites: [],
    sources: [source],
  };
}

async function mountPane(body) {
  apiFetchJson.mockResolvedValue(body);
  const wrapper = mount(TrajectDetailsPane, {
    props: { trajectId: TRAJECT_ID },
    attachTo: document.body,
  });
  // Flush de load() die onMounted aftrapt.
  await wrapper.vm.$nextTick();
  await Promise.resolve();
  await Promise.resolve();
  await wrapper.vm.$nextTick();
  return wrapper;
}

const subpathField = (w) => w.find('nldd-text-field[name="repo_path"]');
const saveButton = (w) =>
  w.findAll('nldd-button').find((b) => b.attributes('text') === 'Opslaan');
// Alleen-lezen waarden staan als `text`-attribuut op een cel: web components
// renderen in deze testomgeving geen shadow-DOM, dus w.text() ziet ze niet.
const cellTexts = (w) =>
  w.findAll('nldd-text-cell').map((c) => c.attributes('text'));

// De opslag-actie hangt een paar microtasks diep: PATCH, lijst verversen,
// detail herladen. Geef ze allemaal de beurt.
async function flush(w) {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve();
    await w.vm.$nextTick();
  }
}

beforeEach(() => {
  roles.clear();
  apiFetch.mockReset();
  apiFetch.mockResolvedValue(undefined);
  apiFetchJson.mockReset();
  // refreshTrajects() in useTrajects doet een kale fetch; die hoort niet bij
  // wat deze test bewijst.
  global.fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => [] });
});

describe('TrajectDetailsPane subpath', () => {
  it('geeft de eigenaar van een traject met eigen repo een veld met Opslaan', async () => {
    const w = await mountPane(detail());
    const field = subpathField(w);
    expect(field.exists()).toBe(true);
    expect(saveButton(w)).toBeTruthy();
    // De hulptekst is dezelfde als in het aanmaakformulier, en de
    // waarschuwing staat er permanent bij - niet pas na een fout.
    expect(w.text()).toContain(
      'Submap met regulation YAML-bestanden. Laat leeg voor repo-root.',
    );
    expect(w.find('nldd-banner').attributes('text')).toBe(
      'Let op: alles buiten deze map ziet de editor niet meer als regulation. Dat geldt ook voor annotaties en documenten van dit traject.',
    );
  });

  it('toont een bijdrager alleen de waarde', async () => {
    const w = await mountPane(
      detail({ role: 'contributor', source: ownSource({ gh_path: 'regulation/nl' }) }),
    );
    expect(subpathField(w).exists()).toBe(false);
    expect(saveButton(w)).toBeUndefined();
    expect(cellTexts(w)).toContain('regulation/nl');
  });

  it('toont het pad van het centrale corpus alleen-lezen, ook voor de eigenaar', async () => {
    const w = await mountPane(
      detail({
        source: ownSource({
          gh_owner: 'MinBZK',
          gh_repo: 'regelrecht-corpus',
          gh_path: 'regulation/nl',
          auth_ref: 'minbzk-central',
        }),
      }),
    );
    expect(subpathField(w).exists()).toBe(false);
    expect(cellTexts(w)).toContain('regulation/nl');
  });

  it('stuurt bij Opslaan een PATCH met repo_path en herlaadt het detail', async () => {
    const w = await mountPane(detail());
    const field = subpathField(w);
    // Zoals het echte veld het doet: de host zet z'n eigen `value` en
    // dispatcht een CustomEvent met diezelfde waarde in `detail`.
    field.element.value = 'regulation/nl';
    field.element.dispatchEvent(
      new CustomEvent('input', {
        detail: { value: 'regulation/nl' },
        bubbles: true,
        composed: true,
      }),
    );
    await w.vm.$nextTick();
    apiFetchJson.mockClear();

    await saveButton(w).trigger('click');
    await flush(w);

    expect(apiFetch).toHaveBeenCalledTimes(1);
    const [url, options] = apiFetch.mock.calls[0];
    expect(url).toBe(`/api/trajects/${TRAJECT_ID}`);
    expect(options.method).toBe('PATCH');
    expect(JSON.parse(options.body)).toEqual({ repo_path: 'regulation/nl' });
    // Criterium: na opslaan wordt het traject-detail opnieuw opgehaald, zodat
    // de genormaliseerde waarde (leeg -> repo-root) in beeld komt.
    expect(apiFetchJson).toHaveBeenCalledWith(
      `/api/trajects/${TRAJECT_ID}`,
      expect.anything(),
    );
  });

  it('laat een geweigerde wijziging als foutmelding bij het veld zien', async () => {
    const w = await mountPane(detail());
    apiFetch.mockRejectedValueOnce(new Error('repo_path moet een relatief pad zijn'));

    await saveButton(w).trigger('click');
    await flush(w);

    expect(w.find('#subpath-error').text()).toContain(
      'repo_path moet een relatief pad zijn',
    );
    expect(subpathField(w).attributes('invalid')).toBeDefined();
    // De foutmelding hangt via `unmet` aan het veld; zonder die verwijzing
    // toont het ontwerpsysteem de lijst niet.
    expect(subpathField(w).attributes('unmet')).toBe(
      w.find('#subpath-error').attributes('id'),
    );

    // Zodra de gebruiker het pad corrigeert verdwijnt de melding, samen met
    // de invalid-markering op het veld.
    const field = subpathField(w);
    field.element.value = 'regulation/nl';
    field.element.dispatchEvent(
      new CustomEvent('input', {
        detail: { value: 'regulation/nl' },
        bubbles: true,
        composed: true,
      }),
    );
    await w.vm.$nextTick();
    expect(subpathField(w).attributes('invalid')).toBeUndefined();
    expect(w.find('#subpath-error').text()).toBe('');
  });
});

const repoField = (w) => w.find('nldd-text-field[name="repo"]');
const repoButton = (w) =>
  w.findAll('nldd-button').find((b) => b.attributes('text') === 'Repo wijzigen');

function typeInto(field, value) {
  field.element.value = value;
  field.element.dispatchEvent(
    new CustomEvent('input', { detail: { value }, bubbles: true, composed: true }),
  );
}

describe('TrajectDetailsPane repo', () => {
  it('geeft een editor-admin een veld met de huidige repo', async () => {
    roles.add('editor-admin');
    const w = await mountPane(detail());
    expect(repoField(w).attributes('value')).toBe('example-org/regelrecht-corpus-example');
    expect(repoButton(w)).toBeTruthy();
  });

  it('geeft een editor-admin het veld ook als bijdrager', async () => {
    roles.add('editor-admin');
    const w = await mountPane(detail({ role: 'contributor' }));
    expect(repoField(w).exists()).toBe(true);
  });

  it('toont eigenaar en bijdrager zonder admin-rol alleen de link', async () => {
    for (const role of ['owner', 'contributor']) {
      const w = await mountPane(detail({ role }));
      expect(repoField(w).exists()).toBe(false);
      expect(repoButton(w)).toBeUndefined();
      w.unmount();
    }
  });

  it('laat de repo van het centrale corpus met rust, ook voor een admin', async () => {
    roles.add('editor-admin');
    const w = await mountPane(
      detail({
        source: ownSource({
          gh_owner: 'MinBZK',
          gh_repo: 'regelrecht-corpus',
          auth_ref: 'minbzk-central',
        }),
      }),
    );
    expect(repoField(w).exists()).toBe(false);
  });

  it('stuurt bij Repo wijzigen een PUT met eigenaar en naam en herlaadt', async () => {
    roles.add('editor-admin');
    const w = await mountPane(detail());
    typeInto(repoField(w), ' other-org/regelrecht-corpus-example ');
    await w.vm.$nextTick();
    apiFetchJson.mockClear();

    await repoButton(w).trigger('click');
    await flush(w);

    expect(apiFetch).toHaveBeenCalledTimes(1);
    const [url, options] = apiFetch.mock.calls[0];
    expect(url).toBe(`/api/trajects/${TRAJECT_ID}/repo`);
    expect(options.method).toBe('PUT');
    expect(JSON.parse(options.body)).toEqual({
      repo_owner: 'other-org',
      repo_name: 'regelrecht-corpus-example',
    });
    expect(apiFetchJson).toHaveBeenCalledWith(`/api/trajects/${TRAJECT_ID}`, expect.anything());
  });

  it('weigert een waarde zonder eigenaar/naam zonder de backend te bellen', async () => {
    roles.add('editor-admin');
    const w = await mountPane(detail());
    typeInto(repoField(w), 'alleen-een-naam');
    await w.vm.$nextTick();

    await repoButton(w).trigger('click');
    await flush(w);

    expect(apiFetch).not.toHaveBeenCalled();
    expect(w.find('#repo-error').text()).toContain('eigenaar/naam');
    expect(repoField(w).attributes('invalid')).toBeDefined();
  });

  it('laat een geweigerde wijziging bij het veld zien', async () => {
    roles.add('editor-admin');
    const w = await mountPane(detail());
    typeInto(repoField(w), 'other-org/regelrecht-corpus-example');
    await w.vm.$nextTick();
    apiFetch.mockRejectedValueOnce(new Error('het geconfigureerde token heeft geen schrijftoegang'));

    await repoButton(w).trigger('click');
    await flush(w);

    expect(w.find('#repo-error').text()).toContain('geen schrijftoegang');
    expect(repoField(w).attributes('unmet')).toBe('repo-error');
  });
});
