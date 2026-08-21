---
node: crate:pipeline
fingerprint: 963947ffd7e0bf5e
---
**Wat.** Een PostgreSQL-backed job queue plus wetstatus-tracking voor het
verwerkingsproces (harvest- en enrich-taken).

**Waarom.** Geeft het inwinnen en verrijken van wetten een betrouwbare,
observeerbare orchestratie: taken worden in de wachtrij gezet, uitgevoerd door
workers en hun status wordt bijgehouden, zodat het proces herstart- en
monitorbaar is.
