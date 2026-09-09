# Doorloop-artefact — Koen en Sadee door het stelsel

Bouwt de pagina waarin beide persona's langs alle regelingen lopen, met onder
elk paneel de redenering die de engine heeft vastgelegd.

## Waarom dit hier staat

De eerste versie (25 augustus 2026) is met de hand gemaakt en had geen
generator. Daardoor liep hij twee feedbackrondes achter: de tekst boven een
paneel viel niet meer samen met de trace eronder. Dat is precies wat dit
artefact waardeloos maakt — de koppeling tussen bewering en bewijs is het punt.

## Draaien

Traces komen uit de BDD-suite, en die schrijft ze **alleen met `TRACE` aan**.
Zonder die vlag draait de suite gewoon groen en blijft `trace_output/` leeg.

```bash
cd packages/engine
TRACE=1 REGULATION_PATH=<corpus> BDD_BUCKET=corpus cargo test --test bdd

cd ../..
python3 docs/financieel-cv/doorloop/genereer.py . koen-en-sadee-doorloop.html
```

De trace-ids in `genereer.py` (`'037'`, `'054'`, …) zijn de volgnummers die de
runner toekent. **Die verschuiven zodra scenario's worden toegevoegd of
hernoemd.** Controleer na een corpuswijziging of elk paneel nog de bedoelde
afleiding toont; het script meldt het niet als een id iets anders is gaan
betekenen. Het draait wel door met een lege plek als een id niet bestaat.

## Onderdelen

| Bestand | Wat |
|---|---|
| `genereer.py` | De generator; bevat ook het verhaal per paneel |
| `style.css` | Overgenomen uit de eerste versie |
| `viewer.js` | Klapt de trace-bomen uit. Rendert pas als het omliggende `details` opengaat |

Het verhaal per paneel staat als spec bovenin `genereer.py` — dat is het
redactionele werk en de plek waar juristfeedback landt.
