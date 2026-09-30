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

## Vergelijken met de vorige versie

`vergelijk.py` zet twee gegenereerde doorlopen naast elkaar: links de oude,
rechts de nieuwe, gekoppeld op wet plus artikel.

```bash
python3 docs/financieel-cv/doorloop/vergelijk.py oud.html nieuw.html vergelijking.html
```

Het onderscheidt een echte wijziging van een andere formulering: alleen wanneer
een uitkomst die in **beide** versies voorkomt van waarde verandert, heet dat
"uitkomst gewijzigd". Verschilt alleen het statuslabel, dan staat er "zelfde
uitkomst, andere benaming". Zonder dat onderscheid markeert de vergelijking
bijna elk paneel als gewijzigd, want de twee versies noemen niet dezelfde set
asserties.

De koppeling moet twee schrijfwijzen overbruggen: de vorige versie zette een
toevoeging achter de wetnaam ("Participatiewet, loonkostensubsidie"), kortte de
Wet WIA af, en voegde artikelnummers samen ("art. 2.1 + 4.1"). Dat zit in
`WET_ALIAS` en `sleutel()`.

## Onderdelen

| Bestand | Wat |
|---|---|
| `genereer.py` | De generator; bevat ook het verhaal per paneel |
| `style.css` | Overgenomen uit de eerste versie |
| `viewer.js` | Klapt de trace-bomen uit. Rendert pas als het omliggende `details` opengaat |
| `vergelijk.py` | Zet twee versies naast elkaar |

Het verhaal per paneel staat als spec bovenin `genereer.py` — dat is het
redactionele werk en de plek waar juristfeedback landt.
