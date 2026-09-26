// De indeling van het zaakscherm. Een zaak kan meer besluiten hebben (een
// voorschot, een vaststelling, een terugvordering); de runtime zegt welke
// besluiten er liggen, en per handeling op welk besluit zij nu handelt
// (`besluit`, het besluitkenmerk). Deze module kent geen besluit bij naam:
// zij groepeert wat de runtime geeft.
import { soortVan, uitkomstTekst } from './tekst.js';

// Of een handeling bij een besluit hoort: zij legde het vast, of zij handelt
// er nu op (een vervolg, een feit dat het volgt, een wijziging ervan).
function hoortBij(h, besluit) {
  return h.naam === besluit.handeling || h.besluit === besluit.besluitkenmerk;
}

// De stand van de feiten met een bedrag (zoals wat er nog te betalen is):
// de uitkomsten van hun proef zonder formulier, dus zoals de zaak nu is.
export function betaalstand(handelingen) {
  const uit = [];
  for (const h of handelingen) {
    if (!h.formulier.some((v) => v.type === 'bedrag')) continue;
    for (const [naam, w] of Object.entries(h.proef?.uitkomsten ?? {})) {
      uit.push({ sleutel: h.naam + naam, handeling: h.label, naam, waarde: uitkomstTekst(w, h.typen?.[naam]) });
    }
  }
  return uit;
}

// De zaak in delen: per besluit zijn handelingen en betaalstand, en de
// handelingen die (nog) bij geen besluit horen, zoals een besluit dat nog
// genomen moet worden of een feit uit het verloop van de zaak.
export function indeling(zaak) {
  const handelingen = zaak?.handelingen ?? [];
  const besluiten = (zaak?.besluiten ?? []).map((b, i) => {
    const eigen = handelingen.filter((h) => hoortBij(h, b));
    const nummer = b.besluitkenmerk.split('/').pop() || String(i + 1);
    return { ...b, nummer, handelingen: eigen, betaalstand: betaalstand(eigen) };
  });
  const overig = handelingen.filter((h) => !besluiten.some((b) => hoortBij(h, b)));
  return { besluiten, overig, betaalstand: betaalstand(overig) };
}

// De stand van een handeling in een regel van de tabel.
export function statusTekst(h) {
  if (soortVan(h) !== 'feit' && h.vastgelegd > 0 && !h.beschikbaar) return 'vastgelegd';
  if (!h.beschikbaar) return 'nog niet';
  return h.vastgelegd > 0 ? `kan (${h.vastgelegd} keer vastgelegd)` : 'kan';
}

// De soort van een handeling in woorden.
export function soortTekst(h) {
  const s = soortVan(h);
  if (s === 'feit') return 'feit';
  return `${s}, stage ${h.stage}`;
}

// De kop van een besluit: welk besluit, wanneer, en wat het wijzigt.
export function besluitKop(b) {
  const delen = [`besluit ${b.besluitkenmerk}`];
  if (b.op_moment) delen.push(`genomen op ${b.op_moment.slice(0, 10)}`);
  if (b.wijzigt) delen.push(`wijzigt besluit ${b.wijzigt}`);
  return delen.join(', ');
}
