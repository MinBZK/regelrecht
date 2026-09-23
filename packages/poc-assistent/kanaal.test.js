import { describe, it, expect } from 'vitest';
import { maakKanaal } from './kanaal.js';

/**
 * Het kanaal draagt het gedrag waar deze wijziging om begon: een gesprek loopt
 * door als je naar een ander tabblad gaat, en wie terugkomt ziet wat hij
 * gemist heeft. Deze tests houden dat vast zonder server en zonder CLI-proces.
 */

/** Een minimale `res`: onthoudt wat erin geschreven is. */
function nepRes() {
  const geschreven = [];
  return {
    geschreven,
    kop: null,
    writeHead(status, headers) { this.kop = { status, headers }; },
    write(tekst) { geschreven.push(tekst); return true; },
    end() { this.gesloten = true; },
    /** De events zoals de browser ze zou parsen. */
    get events() {
      return geschreven
        .filter((r) => r.startsWith('data: '))
        .map((r) => JSON.parse(r.slice(6)));
    },
  };
}

describe('kanaal', () => {
  it('schrijft naar de luisteraar die er is', () => {
    const k = maakKanaal();
    const res = nepRes();
    k.koppel(res);

    k.send({ type: 'tekst', tekst: 'hallo' });

    expect(res.events).toEqual([{ type: 'tekst', tekst: 'hallo' }]);
    expect(res.kop.status).toBe(200);
    expect(res.kop.headers['Content-Type']).toBe('text/event-stream');
  });

  it('bewaart wat er gebeurt terwijl niemand kijkt, en speelt het af', () => {
    const k = maakKanaal();
    k.koppel(nepRes());
    k.ontkoppel();

    k.send({ type: 'tekst', tekst: 'ondertussen' });
    k.send({ type: 'wijziging', document_key: 'wet', toelichting: 'voet omhoog' });

    const terug = nepRes();
    k.koppel(terug);

    expect(terug.events).toEqual([
      { type: 'tekst', tekst: 'ondertussen' },
      { type: 'wijziging', document_key: 'wet', toelichting: 'voet omhoog' },
    ]);
  });

  // Dit is de reden dat de buffer niet alles bewaart: tekst_deel is één
  // gebeurtenis per token. Een doel-run van tien minuten zou er tienduizenden
  // opsparen, en wie niet kijkt ziet het typen toch niet.
  it('bewaart geen losse tekstfragmenten', () => {
    const k = maakKanaal();
    k.koppel(nepRes());
    k.ontkoppel();

    for (let i = 0; i < 100; i++) k.send({ type: 'tekst_deel', tekst: 'x' });
    k.send({ type: 'tekst', tekst: 'de hele beurt' });

    const terug = nepRes();
    k.koppel(terug);

    // De complete beurt blijft, de fragmenten niet: er gaat niets verloren
    // behalve het typen zelf.
    expect(terug.events).toEqual([{ type: 'tekst', tekst: 'de hele beurt' }]);
  });

  it('houdt van voortgang alleen de laatste stand', () => {
    const k = maakKanaal();
    k.koppel(nepRes());
    k.ontkoppel();

    k.send({ type: 'voortgang', beurten: 1, seconden: 5 });
    k.send({ type: 'voortgang', beurten: 2, seconden: 10 });
    k.send({ type: 'voortgang', beurten: 3, seconden: 15 });

    const terug = nepRes();
    k.koppel(terug);

    // Voortgang is een stand en geen gebeurtenis; drie keer dezelfde regel
    // afspelen zegt niets extra's.
    expect(terug.events).toEqual([{ type: 'voortgang', beurten: 3, seconden: 15 }]);
  });

  it('meldt het als er zoveel gemist is dat het niet paste', () => {
    const k = maakKanaal(3);
    k.koppel(nepRes());
    k.ontkoppel();

    for (let i = 1; i <= 5; i++) k.send({ type: 'tekst', tekst: `beurt ${i}` });

    const terug = nepRes();
    k.koppel(terug);

    // De oudste vallen weg, maar de gebruiker hoort te weten dat zijn beeld
    // niet compleet is.
    expect(terug.events[0].tekst).toMatch(/niet bewaard/);
    expect(terug.events.slice(1)).toEqual([
      { type: 'tekst', tekst: 'beurt 3' },
      { type: 'tekst', tekst: 'beurt 4' },
      { type: 'tekst', tekst: 'beurt 5' },
    ]);
  });

  it('speelt het gemiste maar één keer af', () => {
    const k = maakKanaal();
    k.koppel(nepRes());
    k.ontkoppel();
    k.send({ type: 'tekst', tekst: 'eenmalig' });

    k.koppel(nepRes());
    k.ontkoppel();

    const derde = nepRes();
    k.koppel(derde);

    expect(derde.events).toEqual([]);
  });

  // Bij een snelle wissel tussen twee tabbladen komt het sluiten van de oude
  // verbinding ná het aanhaken van de nieuwe binnen. Zonder deze controle zou
  // de oude de verse luisteraar wegtrekken en zag de gebruiker niets meer.
  it('laat een oude luisteraar de nieuwe niet wegtrekken', () => {
    const k = maakKanaal();
    const oud = k.koppel(nepRes());
    const nieuw = nepRes();
    k.koppel(nieuw);

    expect(k.ontkoppel(oud)).toBe(false);
    expect(k.luistert).toBe(true);

    k.send({ type: 'tekst', tekst: 'komt aan' });
    expect(nieuw.events).toEqual([{ type: 'tekst', tekst: 'komt aan' }]);
  });

  it('valt terug op bufferen als schrijven stukloopt', () => {
    const k = maakKanaal();
    const stuk = nepRes();
    stuk.write = () => { throw new Error('verbinding weg'); };
    k.koppel(stuk);

    k.send({ type: 'tekst', tekst: 'niet kwijt' });
    expect(k.luistert).toBe(false);

    const terug = nepRes();
    k.koppel(terug);
    expect(terug.events).toEqual([{ type: 'tekst', tekst: 'niet kwijt' }]);
  });
});
