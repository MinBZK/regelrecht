/**
 * Wijziging doorgeven: één plek waar iemand een verandering in zijn leven
 * meldt, in plaats van per regeling een waarde te corrigeren.
 *
 * De POC had hiervoor een wizard op het dashboard (`FEATURE_CHANGE_WIZARD`,
 * web/templates/partials/dashboard.html) die naar `/edit/update-situation`
 * postte. Die endpoint deed maar één ding: de antwoorden vertalen naar
 * correcties (claims) op de wet die het gegeven bezit — inkomen naar de
 * Wet inkomstenbelasting, adres en huishouden naar de Wet BRP, huur naar de
 * Wet op de huurtoeslag. Dat is precies wat de demo zelf al kan
 * (`demoStore.submitClaim`), dus hier staat alleen die vertaling.
 *
 * Wat het toevoegt boven het corrigeren van een losse waarde: iemand denkt in
 * gebeurtenissen ("ik ben verhuisd", "mijn inkomen is veranderd"), niet in
 * velden van een register. Eén melding raakt meerdere waarden tegelijk, en
 * elke regeling die ervan afhangt rekent meteen opnieuw.
 */

/** Bedragen staan in de registers in centen; het formulier vraagt hele euro's. */
const EUROS = { scale: 100 };

/**
 * De soorten wijziging, elk met de wet die de gegevens bezit en de velden die
 * meegaan. `law` is de wet-id waarop de correctie landt, `keyField` de sleutel
 * waarop die wet haar gegevens opzoekt.
 *
 * Een veld zonder waarde wordt niet ingediend: leeglaten betekent
 * ongewijzigd, niet nul.
 */
export const CHANGE_TYPES = [
  {
    id: 'inkomen',
    label: 'Mijn inkomen of vermogen',
    icon: 'euro-sign',
    description: 'Loon, winst, uitkering, spaargeld of schulden',
    law: 'wet_inkomstenbelasting',
    keyField: 'bsn',
    groups: [
      {
        label: 'Box 1 — werk en woning',
        fields: [
          { name: 'loon_uit_dienstbetrekking', label: 'Loon uit dienstbetrekking', ...EUROS },
          { name: 'uitkeringen_en_pensioenen', label: 'Uitkeringen en pensioenen', ...EUROS },
          { name: 'winst_uit_onderneming', label: 'Winst uit onderneming', ...EUROS },
          { name: 'resultaat_overige_werkzaamheden', label: 'Resultaat overige werkzaamheden', ...EUROS },
          { name: 'eigen_woning', label: 'Inkomsten uit eigen woning', ...EUROS },
        ],
      },
      {
        label: 'Box 2 — aanmerkelijk belang',
        fields: [
          { name: 'reguliere_voordelen', label: 'Reguliere voordelen (dividend)', ...EUROS },
          { name: 'vervreemdingsvoordelen', label: 'Vervreemdingsvoordelen', ...EUROS },
        ],
      },
      {
        label: 'Box 3 — sparen en beleggen',
        fields: [
          { name: 'spaargeld', label: 'Spaargeld', ...EUROS },
          { name: 'beleggingen', label: 'Beleggingen', ...EUROS },
          { name: 'onroerend_goed', label: 'Onroerend goed', ...EUROS },
          { name: 'schulden', label: 'Schulden', ...EUROS },
        ],
      },
    ],
  },
  {
    id: 'huurprijs',
    label: 'Mijn huur',
    icon: 'house',
    description: 'Huurprijs en servicekosten',
    law: 'wet_op_de_huurtoeslag',
    keyField: 'bsn',
    groups: [
      {
        label: 'Wat u per maand betaalt',
        fields: [
          { name: 'huurprijs', label: 'Kale huurprijs per maand', ...EUROS },
          { name: 'servicekosten', label: 'Servicekosten per maand', ...EUROS },
          { name: 'subsidiabele_servicekosten', label: 'Waarvan subsidiabel', ...EUROS },
        ],
      },
    ],
  },
  {
    id: 'woonadres',
    label: 'Mijn adres',
    icon: 'location',
    description: 'Verhuizing of een correctie op uw adres',
    law: 'wet_brp',
    keyField: 'bsn',
    groups: [
      {
        label: 'Uw nieuwe adres',
        fields: [
          { name: 'straat', label: 'Straatnaam', kind: 'text', part: 'adres' },
          { name: 'huisnummer', label: 'Huisnummer', kind: 'text', part: 'adres' },
          { name: 'postcode', label: 'Postcode', kind: 'text', part: 'adres' },
          { name: 'woonplaats', label: 'Woonplaats', kind: 'text', part: 'adres' },
        ],
      },
    ],
  },
  {
    id: 'huishouden',
    label: 'Mijn huishouden',
    icon: 'person-2',
    description: 'Trouwen, scheiden, samenwonen of een kind',
    law: 'wet_brp',
    keyField: 'bsn',
    // Een gebeurtenis, geen bedrag: de keuze bepaalt wat er verandert.
    events: [
      {
        value: 'scheiden',
        label: 'Ik ga scheiden of wij gaan uit elkaar',
        icon: 'person-badge-minus',
        // De POC ondersteunde alleen deze; de andere kwamen terug als
        // "Deze flow is nog niet ondersteund". Hier gebeurt hetzelfde,
        // maar dan zichtbaar vóór het indienen in plaats van erna.
        changes: { partnerschap_type: 'GEEN', partner_bsn: null },
      },
      {
        value: 'samenwonen',
        label: 'Ik ga trouwen of samenwonen',
        icon: 'heart',
        unsupported: 'Hiervoor is de gegevens van uw partner nodig; dat kan in deze demo nog niet.',
      },
      { value: 'kind', label: 'Ik krijg een kind', icon: 'person-badge-plus', unsupported: 'Een geboorte melden kan in deze demo nog niet.' },
      { value: 'iemand-bij', label: 'Er komt iemand bij mij wonen', icon: 'person-badge-plus', unsupported: 'Een medebewoner melden kan in deze demo nog niet.' },
      { value: 'iemand-weg', label: 'Er gaat iemand mijn huis uit', icon: 'person-badge-minus', unsupported: 'Een vertrekkende medebewoner melden kan in deze demo nog niet.' },
      { value: 'overlijden', label: 'Er is iemand overleden', icon: 'person-badge-minus', unsupported: 'Een overlijden melden kan in deze demo nog niet.' },
    ],
  },
];

export function changeTypeById(id) {
  return CHANGE_TYPES.find((t) => t.id === id) ?? null;
}

/** Alle velden van een soort wijziging, plat. */
export function fieldsOf(type) {
  return (type?.groups ?? []).flatMap((g) => g.fields);
}

/**
 * Een ingevuld formulier omzetten naar de correcties die ingediend worden.
 *
 * Leeg betekent ongewijzigd, dus die velden vallen weg. Een bedrag gaat als
 * centen naar de wet, want zo staat het in het register. Het adres is één
 * samengestelde waarde (`adres`, kind: record in bindings.yaml) plus het
 * verblijfsadres als tekst, precies zoals `/edit/update-situation` het
 * samenstelde.
 *
 * @returns {Array<{law: string, input: string, value: any, label: string}>}
 */
export function claimsFromAnswers(type, answers) {
  if (!type) return [];
  const filled = (name) => {
    const v = answers?.[name];
    return v !== undefined && v !== null && String(v).trim() !== '';
  };

  if (type.id === 'huishouden') {
    const event = (type.events ?? []).find((e) => e.value === answers?.event);
    if (!event || event.unsupported || !event.changes) return [];
    return Object.entries(event.changes).map(([input, value]) => ({
      law: type.law,
      input,
      value,
      label: event.label,
    }));
  }

  if (type.id === 'woonadres') {
    const parts = fieldsOf(type).filter((f) => filled(f.name));
    if (!parts.length) return [];
    const adres = Object.fromEntries(parts.map((f) => [f.name, String(answers[f.name]).trim()]));
    const claims = [{ law: type.law, input: 'adres', value: { ...adres, type: 'WOONADRES' }, label: 'Adres' }];
    // De wet leest het adres ook als één regel; die blijft anders op het oude staan.
    const street = adres.straat && adres.huisnummer ? `${adres.straat} ${adres.huisnummer}` : null;
    const city = adres.postcode && adres.woonplaats ? `${adres.postcode} ${adres.woonplaats}` : adres.woonplaats ?? null;
    const full = [street, city].filter(Boolean).join(', ');
    if (full) claims.push({ law: type.law, input: 'verblijfsadres', value: full, label: 'Verblijfsadres' });
    return claims;
  }

  return fieldsOf(type)
    .filter((f) => filled(f.name))
    .map((f) => {
      const raw = Number(String(answers[f.name]).replace(',', '.'));
      return {
        law: type.law,
        input: f.name,
        value: f.scale ? Math.round(raw * f.scale) : raw,
        label: f.label,
      };
    })
    .filter((c) => Number.isFinite(c.value));
}
