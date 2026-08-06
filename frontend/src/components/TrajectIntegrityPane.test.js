import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import TrajectIntegrityPane from './TrajectIntegrityPane.vue';
import { groupBySeverity } from '../composables/useTrajectIntegrity.js';

// De pane haalt het rapport via useTrajectIntegrity -> apiFetchJson op; die
// ene netwerkpoot sturen we hier (zelfde mock-vorm als TasksCategoriesPane).
const apiFetchJson = vi.fn();
vi.mock('../lib/apiFetch.js', () => ({
  apiFetchJson: (...a) => apiFetchJson(...a),
}));

// Fictieve wetten en mapnamen: dit is een publieke repo, dus geen echte
// traject- of repo-namen in fixtures.
const ERROR_FINDING = {
  severity: 'error',
  kind: 'directory_name_mismatch',
  path: 'waterschaps_verordening/hoogland/keur_alpha',
  law_id: 'keur_alpha_hoogland',
  message: "De wet in de map 'keur_alpha' heeft '$id: keur_alpha_hoogland'.",
  remedy: "Hernoem de map naar 'keur_alpha_hoogland'.",
};
const WARNING_FINDING = {
  severity: 'warning',
  kind: 'scenario_directory_without_target',
  path: 'wet/wet_alpha/scenarios',
  law_id: 'wet_alpha',
  message: "De scenario's evalueren 'wet_alpha' niet.",
  remedy: 'Voeg een evaluatiestap toe.',
};

const CLEAN_REPORT = {
  traject_ref: 'voorbeeldtraject-abcd1234',
  source_id: 'traject-own',
  checked_laws: 12,
  checked_scenarios: 3,
  findings: [],
};
const DIRTY_REPORT = {
  ...CLEAN_REPORT,
  findings: [ERROR_FINDING, WARNING_FINDING],
};

async function mountPane() {
  const wrapper = mount(TrajectIntegrityPane, {
    props: { trajectRef: 'voorbeeldtraject-abcd1234' },
    attachTo: document.body,
  });
  // Flush de load() die onMounted aftrapt.
  await wrapper.vm.$nextTick();
  await Promise.resolve();
  await Promise.resolve();
  await wrapper.vm.$nextTick();
  return wrapper;
}

describe('groupBySeverity', () => {
  it('zet fouten boven waarschuwingen en laat lege groepen weg', () => {
    const groups = groupBySeverity({ findings: [WARNING_FINDING, ERROR_FINDING] });
    expect(groups.map((g) => g.severity)).toEqual(['error', 'warning']);
    expect(groups[0].title).toBe('Fouten');
    expect(groups[0].findings).toEqual([ERROR_FINDING]);
  });

  it('geeft een lege lijst voor een leeg of ontbrekend rapport', () => {
    expect(groupBySeverity(null)).toEqual([]);
    expect(groupBySeverity(CLEAN_REPORT)).toEqual([]);
  });

  it('laat een onbekende severity niet verdwijnen maar achteraan belanden', () => {
    const groups = groupBySeverity({
      findings: [{ ...WARNING_FINDING, severity: 'toekomstig' }, ERROR_FINDING],
    });
    expect(groups.map((g) => g.severity)).toEqual(['error', 'toekomstig']);
  });
});

describe('TrajectIntegrityPane', () => {
  beforeEach(() => {
    vi.resetModules();
    apiFetchJson.mockReset();
    document.body.innerHTML = '';
  });

  it('vraagt het rapport op voor het traject uit de props', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    await mountPane();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);
    expect(apiFetchJson.mock.calls[0][0]).toBe(
      '/api/trajects/voorbeeldtraject-abcd1234/integrity',
    );
  });

  it('rendert de bevindingen gegroepeerd op severity, met omschrijving en remedie', async () => {
    apiFetchJson.mockResolvedValue(DIRTY_REPORT);
    const wrapper = await mountPane();
    const html = wrapper.html();

    // Beide koppen, met het aantal erbij.
    expect(html).toContain('Fouten (1)');
    expect(html).toContain('Waarschuwingen (1)');
    // Fouten staan boven waarschuwingen.
    expect(html.indexOf('Fouten (1)')).toBeLessThan(html.indexOf('Waarschuwingen (1)'));

    // Elke bevinding draagt omschrijving + remedie + het pad waar hij zit.
    const cells = wrapper.findAll('nldd-text-cell');
    const error = cells.find((c) => c.attributes('text') === ERROR_FINDING.message);
    expect(error).toBeTruthy();
    expect(error.attributes('supporting-text')).toBe(ERROR_FINDING.remedy);
    expect(error.attributes('overline')).toBe(ERROR_FINDING.path);

    const warning = cells.find((c) => c.attributes('text') === WARNING_FINDING.message);
    expect(warning.attributes('supporting-text')).toBe(WARNING_FINDING.remedy);

    // Geen "alles in orde"-melding als er wél iets is.
    expect(html).not.toContain('Geen problemen gevonden');
  });

  it('toont een bevestigende lege staat als er niets mis is', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    const wrapper = await mountPane();

    const empty = wrapper
      .findAll('nldd-inline-dialog')
      .find((d) => d.attributes('text') === 'Geen problemen gevonden');
    expect(empty).toBeTruthy();
    expect(empty.attributes('variant')).toBe('success');
    // De omvang van de controle staat erbij, anders leest "geen problemen"
    // als "er is niets gecontroleerd".
    expect(empty.attributes('supporting-text')).toContain('12 wetbestanden');
    expect(empty.attributes('supporting-text')).toContain("3 scenario's");
  });

  it('toont de foutmelding van de backend als de controle niet kon draaien', async () => {
    apiFetchJson.mockRejectedValue(new Error('GitHub is nu niet bereikbaar.'));
    const wrapper = await mountPane();

    const alert = wrapper
      .findAll('nldd-inline-dialog')
      .find((d) => d.attributes('variant') === 'alert');
    expect(alert).toBeTruthy();
    expect(alert.attributes('text')).toBe('Integriteitscontrole niet gelukt');
    expect(alert.attributes('supporting-text')).toBe('GitHub is nu niet bereikbaar.');
    // Geen rapport, dus ook geen lege staat die suggereert dat alles klopt.
    expect(wrapper.html()).not.toContain('Geen problemen gevonden');
  });

  it('haalt het rapport opnieuw op via de verversknop', async () => {
    apiFetchJson.mockResolvedValue(CLEAN_REPORT);
    const wrapper = await mountPane();
    expect(apiFetchJson).toHaveBeenCalledTimes(1);

    const refresh = wrapper
      .findAll('nldd-button')
      .find((b) => b.attributes('text') === 'Opnieuw controleren');
    expect(refresh).toBeTruthy();
    await refresh.trigger('click');
    await wrapper.vm.$nextTick();
    expect(apiFetchJson).toHaveBeenCalledTimes(2);
  });
});
