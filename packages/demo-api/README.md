# regelrecht-demo-api

Minimale LLM-uitlegproxy voor de burger-demo. Eén route (`POST /api/explain`) die een law-uitvoer + trace vertaalt naar een burger-vriendelijke Nederlandse uitleg via de Anthropic API. Zie [RFC-016](../../docs/rfcs/rfc-016.md).

## Env

- `ANTHROPIC_API_KEY` — **vereist**. Proces stopt zonder deze.
- `ANTHROPIC_MODEL` — default `claude-opus-4-7`.
- `ALLOWED_ORIGINS` — comma-separated allow-list. Default `http://localhost:7180`. Entries kunnen exact zijn (`https://demo.regelrecht.rijks.app`) of een subdomein-wildcard `*.regelrecht.rijks.app` (matcht subdomeinen, NIET het apex). Legacy `ALLOWED_ORIGIN` (enkelvoud) wordt nog gehonoreerd.
- `PORT` — default `7181`.

## Routes

| Methode | Pad | Beschrijving |
|---|---|---|
| GET | `/health` | Health-check |
| POST | `/api/explain` | LLM-uitleg; rate-limited 5/min per IP |

Request-body van `/api/explain`:

```json
{
  "law_id": "zorgtoeslagwet",
  "law_label": "Zorgtoeslag",
  "output_name": "hoogte_zorgtoeslag",
  "parameters": { "bsn": "100000001" },
  "result": { "outputs": { "hoogte_zorgtoeslag": 12500 } },
  "trace": { "steps": ["…"] },
  "profile_summary": "Alleenstaande ZZP'er, geen partner, inkomen €30.000"
}
```

Response:

```json
{ "explanation": "…", "model": "claude-opus-4-7" }
```

## Lokaal draaien

```bash
ANTHROPIC_API_KEY=sk-ant-… just demo-api
# of
ANTHROPIC_API_KEY=sk-ant-… cd packages && cargo run --package regelrecht-demo-api
```

De frontend-demo proxiet `/api/*` naar poort 7181 (zie `frontend-demo/vite.config.js`).
