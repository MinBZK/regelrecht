/**
 * Het kanaal tussen een lopend gesprek en wie er op dat moment naar kijkt.
 *
 * Apart bestand omdat dit het stuk is dat het gedrag draagt waar het om
 * begon: een gesprek loopt door als je naar een ander tabblad gaat. Hier is
 * het los te testen, zonder server en zonder CLI-proces.
 */

/**
 * Hoeveel gemiste gebeurtenissen een gesprek bewaart voor wie terugkomt.
 *
 * `tekst_deel` gaat hier nooit in: dat is één gebeurtenis per token, en wie
 * niet kijkt ziet de tekst toch niet verschijnen. De complete beurt komt als
 * `tekst` alsnog voorbij, dus er gaat niets verloren behalve het typen zelf.
 */
export const BUFFER_MAX = 500;

/**
 * Het kanaal tussen een gesprek en wie er op dat moment naar kijkt.
 *
 * Het gesprek schrijft naar het kanaal en weet niet of er iemand is. Is er
 * niemand, dan onthoudt het kanaal wat er gebeurde; komt er iemand (terug),
 * dan krijgt die eerst het gemiste en daarna live verder.
 *
 * Wat niet bewaard wordt: `tekst_deel`. Dat is één gebeurtenis per token, en
 * wie niet kijkt ziet het typen toch niet. De afgeronde beurt komt als `tekst`
 * alsnog langs, dus de inhoud blijft compleet. Zonder deze uitzondering zou
 * een doel-run van tien minuten tienduizenden fragmenten opsparen.
 */
export function maakKanaal(bufferMax = BUFFER_MAX) {
  let res = null;
  const gemist = [];
  let afgekapt = false;

  const schrijf = (event) => {
    try {
      res.write(`data: ${JSON.stringify(event)}\n\n`);
      return true;
    } catch {
      // De verbinding is onderweg weggevallen; verder als losgekoppeld.
      res = null;
      return false;
    }
  };

  return {
    get luistert() { return res !== null; },

    send(event) {
      if (res && schrijf(event)) return;
      if (event.type === 'tekst_deel') return;
      // Voortgang is een stand, geen gebeurtenis: er komt er elke vijf seconden
      // een, en alleen de laatste zegt nog iets. Zonder dit loopt de buffer vol
      // met verouderde kopieën van hetzelfde.
      if (event.type === 'voortgang') {
        const laatste = gemist[gemist.length - 1];
        if (laatste?.type === 'voortgang') gemist[gemist.length - 1] = event;
        else gemist.push(event);
        return;
      }
      if (gemist.length >= bufferMax) {
        // Liever de oudste laten vallen dan geheugen laten groeien; de melding
        // hieronder zorgt dat de app weet dat het beeld niet compleet is.
        gemist.shift();
        afgekapt = true;
      }
      gemist.push(event);
    },

    /**
     * Zet een nieuwe luisteraar op het kanaal en speel het gemiste af.
     * Geeft die luisteraar terug, zodat `ontkoppel` kan controleren of hij nog
     * de huidige is: bij een snelle wissel tussen twee tabbladen komt het
     * sluiten van de oude verbinding ná het aanhaken van de nieuwe binnen, en
     * zonder die controle trekt de oude de verse luisteraar weg.
     */
    koppel(nieuweRes) {
      nieuweRes.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        Connection: 'keep-alive',
      });
      res = nieuweRes;
      if (afgekapt) {
        schrijf({
          type: 'tekst',
          tekst: 'Een deel van wat er ondertussen gebeurde is niet bewaard; hieronder gaat het verder.',
        });
        afgekapt = false;
      }
      while (gemist.length) schrijf(gemist.shift());
      return nieuweRes;
    },

    /**
     * Deze luisteraar is weg; wat nu binnenkomt gaat de buffer in. Alleen als
     * hij ook werkelijk de huidige is (zie `koppel`).
     */
    ontkoppel(welke = null) {
      if (welke !== null && res !== welke) return false;
      res = null;
      return true;
    },

    /** Sluit af, als er iemand is om het tegen te zeggen. */
    sluit() {
      try { res?.end(); } catch { /* al dicht */ }
      res = null;
    },
  };
}
