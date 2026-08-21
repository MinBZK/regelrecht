---
node: crate:engine
fingerprint: 31375e4c860d7f37
---
**Wat.** De uitvoeringsmotor voor machine-leesbare Nederlandse wetten. De engine
leest een wet-YAML, resolveert de invoer en cross-law bronnen, en berekent de
uitkomsten van een regeling voor een gegeven situatie.

**Waarom.** Dit is het hart van regelrecht: het scheidt de *uitvoeringslogica*
van de *wetteksten*, zodat wetten als data behandeld kunnen worden en dezelfde
motor elke regeling uitvoert. De engine conformeert aan het canonieke
`law-model` en deelt domeintypen via `shared`.
