# Keten-checkpoints — assert elke knoop, niet alleen de endpoint

De engine berekent tussen de leaf-feiten en de endpoint een **keten** van tussen-normen.
Wie alleen de endpoint assert, test de keten niet: een foute keten die op de gesamplede
casus toevallig dezelfde endpoint oplevert, slipt er geluidloos door. Keten-checkpoints
maken elke knoop op het kritieke pad expliciet uitgesproken.

## Het kritieke pad afleiden uit de YAML

1. Begin bij de endpoint-output van het scenario.
2. Volg in de YAML de bindingen terug: elke `output`/tussen-norm hangt af van andere
   outputs, van leaf-parameters, en (bij cross-law) van outputs van andere wetten via
   `source`. Dit vormt een gerichte acyclische graaf (DAG).
3. Voor een gegeven persona is het **kritieke pad** de subset knopen die de uitkomst
   daadwerkelijk bepalen — inclusief de knopen die een uitsluiting *neutraliseren* of een
   tak *afsluiten* (een tegen-uitsluiting die "uit" staat is net zo bepalend als één die
   "aan" staat).
4. Leg de DAG + per-persona-pad vast in `templates/keten-kaart.md`.

## De conventie

Elk scenario assert **elke knoop op zijn kritieke pad**, niet alleen de endpoint. Dus
naast de eind-output ook de relevante tussen-normen: de hoofdgrond die geldt, elke
uitsluiting die wel/niet van toepassing is, en elke tegen-uitsluiting die wel/niet
neutraliseert. Een lezer ziet dan de juridische structuur in het scenario zelf; een
ketenfout flipt een checkpoint, ook als de endpoint toevallig gelijk blijft.

Houd het **kritiek**: assert de knopen die de uitkomst dragen, niet elke berekende
waarde. Te veel checkpoints maken het scenario bros en onleesbaar; te weinig laten gaten.
De keten-kaart helpt kiezen: assert de knopen waar paden splitsen of samenkomen.

## Cross-law-ketens

Bij een keten over meerdere wetten: assert minstens de **scharnierpunten** — de output
van elke wet die de volgende wet ingaat. Een cross-law-binding die alleen in de endpoint
zichtbaar wordt, verbergt waar in de keten het misgaat. (Forwarding van leaf-feiten over
meerdere hops is een bekende valkuil: elke hop moet zijn eigen inputs doorkrijgen — een
checkpoint per scharnier maakt een gebroken hop direct zichtbaar.)

## Sensitiviteits-/twin-scenario's

Een checkpoint bewijst dat een knoop *een* waarde heeft; een **sensitiviteitstest**
bewijst dat de knoop op het *juiste* leaf reageert. Bouw scenario-paren die op precies
één as-waarde verschillen (zie de twin-persona's in `persona-bibliotheek.md`), gekozen
zó dat het verschil door één ketenschakel loopt:

- Als de schakel correct is, flipt exact het verwachte checkpoint (en alleen dat).
- Als de schakel fout is (verkeerde EN/OF, gemiste voorwaarde, verkeerde forwarding),
  flipt het verkeerde of geen checkpoint.

Dekt zo de scharnier-logica af: conjuncties (beide voorwaarden nodig), disjuncties (één
voldoende), en neutralisaties (de tegen-uitsluiting doet wat hij hoort).

## Band met de features-vs-YAML meta-check

`regelrecht-stelselanalyse` waarschuwt: *als de scenario's en de YAML uit dezelfde
(mogelijk foute) interpretatie komen, bewijzen groene tests geen juridische
correctheid.* Keten-checkpoints alleen lossen dat niet op — ze komen nog steeds uit jouw
interpretatie. Wat ze wél doen: ze maken de interpretatie **expliciet en
controleerbaar** per knoop, zodat een externe verificatie (wettekst, nota van
toelichting, expert) elke schakel los kan toetsen in plaats van alleen de endpoint. Leg
de verwachte checkpoint-waarden naast de wettekst, niet alleen naast de YAML.

## Resultaat

Per endpoint een keten-kaart met geannoteerde knopen, en scenario's die hun pad-knopen
asserten + twin-paren voor de scharnieren. Dit is de leesbare, falsifieerbare uitvoerkant
die golden traces (`golden-traces.md`) vervolgens dichttimmeren tegen regressie.
