/**
 * graafUitMetrieken: maakt de graafvorm uit het rapport van `corpusMetrics()`.
 *
 * De graaf tekende tot nu toe op `lib/corpusgraaf.js`, dat de YAML in de
 * frontend opnieuw las. Die tweede lezer verdwijnt hiermee uit het productpad:
 * knopen en randen komen voortaan uit hetzelfde rapport als de tegels, dus de
 * graaf kan niet meer iets anders beweren dan de cijfers erboven.
 *
 * ## Eén knoop per regeling, niet per versie
 *
 * Het rapport levert een rij per geladen *versie*. Een wet met drie versies
 * geeft dus drie rijen met hetzelfde `law_id`. Die één op één als knoop
 * overnemen levert dubbele knoop-ids op, en Vue Flow tekent er dan willekeurig
 * één van. De graaf gaat over samenhang tussen regelingen, niet tussen versies,
 * dus we vouwen ze samen: de nieuwste versie levert de metadata, en een
 * regeling heet pas niet-aangeroepen als *geen enkele* van haar versies een
 * inkomende binding heeft.
 */

/** Nieuwste eerst, op `valid_from`. Een versie zonder datum telt als oudste. */
function nieuwsteEerst(a, b) {
  return String(b.valid_from ?? '').localeCompare(String(a.valid_from ?? ''));
}

/**
 * @param {object} rapport uitvoer van `engine.corpusMetrics()`
 * @returns {{knopen: object[], randen: object[], telling: object}} vorm die
 *   `CorpusGraafView` verwacht
 */
export function graafUitMetrieken(rapport) {
  const regelingen = rapport?.regulations ?? [];
  const bindingen = rapport?.bindings ?? [];

  const perWet = new Map();
  for (const r of regelingen) {
    const bestaand = perWet.get(r.law_id) ?? [];
    bestaand.push(r);
    perWet.set(r.law_id, bestaand);
  }

  const knopen = [...perWet.entries()]
    .sort(([a], [b]) => a.localeCompare(b, 'nl'))
    .map(([lawId, versies]) => {
      const [nieuwste] = [...versies].sort(nieuwsteEerst);
      const inkomend = versies.reduce((som, v) => som + (v.incoming_bindings ?? 0), 0);
      const geladen = versies.some((v) => v.loaded);
      return {
        lawId,
        aanwezig: geladen,
        laag: nieuwste.layer ?? null,
        validFrom: nieuwste.valid_from ?? null,
        // Artikelen en outputs van de nieuwste versie, niet de som over versies:
        // optellen zou een wet met drie versies drie keer zo groot laten lijken.
        artikelen: nieuwste.article_count ?? 0,
        outputs: nieuwste.output_count ?? 0,
        inkomend,
        nietAangeroepen: geladen && inkomend === 0,
      };
    });

  const randen = bindingen
    .map((b) => ({
      van: b.from_law,
      naar: b.to_law,
      soort: b.kind,
      integriteit: b.integrity,
      label: b.label ?? '',
    }))
    .sort(
      (a, b) =>
        a.van.localeCompare(b.van, 'nl') ||
        a.naar.localeCompare(b.naar, 'nl') ||
        a.label.localeCompare(b.label, 'nl'),
    );

  const klassen = rapport?.totals?.findings_by_class ?? {};
  return {
    knopen,
    randen,
    telling: {
      clean: rapport?.totals?.bindings_clean ?? 0,
      misplaced: klassen.misplaced ?? 0,
      dangling: klassen.dangling ?? 0,
      'plain-param': klassen['plain-param'] ?? 0,
      'impl-dangling': klassen['impl-dangling'] ?? 0,
      'impl-no-date': klassen['impl-no-date'] ?? 0,
    },
  };
}
