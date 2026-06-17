# Service-spec — afleiding, contract & verpakking

De **service-spec** is het enige casus-specifieke bestand: één gegenereerde configuratie die
de PoC stuurt. De referentie-architectuur (app-code) is 100% casus-agnostisch; per wet
verschilt alleen deze spec + de redactionele B1-teksten. Het corpus blijft by-reference.

## 1. Afleiding — corpus-signaal → service-spec

Geen vaste archetype-lijst; leid de dienst af uit wat de YAML's impliceren. Het archetype is
een *emergente samenvatting*, geen schakelaar.

| Corpus-signaal | Implicatie voor de dienst |
|---|---|
| `hooks` (AANVRAAG / BEHANDELING / BESLUIT / BEKENDMAKING / BEZWAAR) | de levenscyclus-fasen → procesflow + schermen |
| `legal_character` (besluit) + `produces` | er is een besluit → motiveringsplicht, bekendmaking, bezwaar/beroep |
| `untranslatables` | waar menselijk oordeel zit → de mens-taken-wachtrij |
| caller-params (leaf-inputs) + herkomst-classificatie | welke gegevens nodig zijn en *van wie* (burger / ander systeem / oordeel) → de last, en of een burgerportaal zinvol is vs. vooraf-invullen |
| `source:` / `implements:` | ketensamenwerking → andere partijen/systemen, data-herkomst |
| `type_spec` (days / weeks / bedrag-eenheden) | termijnen & bedragen in de dienst |
| aanwezigheid initiatie-param + AANVRAAG-hook | initiatie: burger-geïnitieerd vs. ambtshalve vs. melding |

**Emergente archetypen** (samenvattingen, geen invoer): aanvraag→beschikking · melding/
registratie · ambtshalve oplegging · **headless beslis-service** (geen eigen portaal — een
beslis-/uitleg-/trace-API als formele plugin op een ander systeem). Vindt de afleiding geen
burger-initiatie → `lenzen: [headless]`.

## 2. Gereedheid-contract op het corpus

Minimaal nodig om zinvol te genereren: hooks aanwezig, besluit-karakter gezet waar van
toepassing, caller-params met herkomst-hint, type_spec-eenheden op termijn-/bedrag-outputs.
Ontbreekt iets → **soepel genereren met markeringen** en het gat op de bevindingenlijst
zetten (afwezigheid is een bevinding). Niet weigeren.

## 3. Het contract (neutraal voorbeeld)

Zie `templates/service-spec.example.yaml` voor een volledig, **neutraal** voorbeeld met
placeholders. De spec bevat o.a.: `lenzen`, `levenscyclus`, `formulier` (groepen/velden met
`herkomst` + B1-`label`, gates, escalaties), `keten` (knoop → artikel), `termijnen`,
`mens_taken_bron`, `wat_als`, `open_vragen`, `herkomst_annotaties`.

## 4. Verpakking — drie lagen

| Laag | Inhoud | Per casus? |
|---|---|---|
| Referentie-architectuur (de app) | engine=rekenmeester, levenscyclus-statemachine, lenzen/views, componenten, dialoog-flow, auth-adapter, receipts, guards, testharnas | nee — nooit |
| Service-spec (`service.config.yaml`) | params/labels, lenzen, levenscyclus, keten, termijnen, mens-taken-bron, wat-als | ja — gegenereerd |
| Corpus | de wet-YAML's | by-reference |

## 5. Onderhoudbaarheids-eisen aan de template

De template moet deze inbakken (een naïef handgebouwde PoC schendt ze typisch):

1. **Formulier-metadata uit corpus/engine** i.p.v. hardcoded veldgroepen/labels/param-namen.
2. **Getypeerd API-contract** (gedeelde types / OpenAPI) i.p.v. ongetypeerde respons.
3. **Regressienet**: round-trip-tests op de conversies, keten-checkpoint-waarden, en een
   **corpus-contracttest** (faalt zodra app en corpus uiteenlopen).
4. **Beslis-/mapping-logica richting corpus** waar mogelijk (niet in de backend hardcoden).
5. **Engine gepind op release** + corpus-versie in de receipt; **migratieframework**; echte
   DB/sessiestore.
6. **Echte design-system-componenten** + geverifieerde toegankelijkheid.
