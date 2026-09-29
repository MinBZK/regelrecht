// De indeling van het zaakscherm. Een zaak kan meer besluiten hebben (een
// voorschot, een vaststelling, een terugvordering); de runtime zegt welke
// besluiten er liggen, en per handeling op welk besluit zij nu handelt
// (`decision`, het id van het besluitgram). Deze module kent geen besluit bij naam:
// zij groepeert wat de runtime geeft.
import { soortVan, uitkomstTekst } from './tekst.js';

// Of een handeling bij een besluit hoort: zij legde het vast, of zij handelt
// er nu op (een vervolg, een feit dat het volgt, een wijziging ervan).
function hoortBij(h, besluit) {
  return h.name === besluit.action || h.decision === besluit.id;
}

// De stand van de feiten met een bedrag (zoals wat er nog te betalen is):
// de uitkomsten van hun proef zonder formulier, dus zoals de zaak nu is.
export function betaalstand(handelingen) {
  const uit = [];
  for (const h of handelingen) {
    if (!h.form.some((v) => v.type === 'amount')) continue;
    for (const [naam, w] of Object.entries(h.trial?.outputs ?? {})) {
      uit.push({ sleutel: h.name + naam, handeling: h.label, naam, waarde: uitkomstTekst(w, h.types?.[naam]) });
    }
  }
  return uit;
}

// De zaak in delen: per besluit zijn handelingen en betaalstand, en de
// handelingen die (nog) bij geen besluit horen, zoals een besluit dat nog
// genomen moet worden of een feit uit het verloop van de zaak.
export function indeling(zaak) {
  const handelingen = zaak?.actions ?? [];
  const besluiten = (zaak?.decisions ?? []).map((b, i) => {
    const eigen = handelingen.filter((h) => hoortBij(h, b));
    // Het hoeveelste besluit in de groep: het gram zelf heeft alleen een id.
    const nummer = String(i + 1);
    return { ...b, nummer, handelingen: eigen, betaalstand: betaalstand(eigen) };
  });
  const overig = handelingen.filter((h) => !besluiten.some((b) => hoortBij(h, b)));
  return { besluiten, overig, betaalstand: betaalstand(overig) };
}

// De stand van een handeling in een regel van de tabel.
export function statusTekst(h) {
  if (soortVan(h) !== 'fact' && h.recorded > 0 && !h.available) return 'vastgelegd';
  if (!h.available) return 'nog niet';
  return h.recorded > 0 ? `kan (${h.recorded} keer vastgelegd)` : 'kan';
}

// De soort van een handeling in woorden.
const SOORT = { fact: 'feit', decision: 'besluit', follow_up: 'vervolg' };

export function soortTekst(h) {
  const s = soortVan(h);
  if (s === 'fact') return SOORT.fact;
  return `${SOORT[s] ?? s}, stage ${h.stage}`;
}

// De kop van een besluit: welk besluit, wanneer, en wat het wijzigt.
export function besluitKop(b) {
  const delen = [`besluit ${b.id}`];
  if (b.effective_at) delen.push(`genomen op ${b.effective_at.slice(0, 10)}`);
  if (b.amends) delen.push(`wijzigt besluit ${b.amends}`);
  return delen.join(', ');
}
