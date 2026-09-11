#!/bin/sh
# Start het portaal en, als er een token is, een beleidsassistent per casus.
#
# Waarom een scriptje en niet drie containers: de assistent is geen dienst maar
# een demo-hulpje dat de Claude CLI aanroept met de tools van één casus. Hij
# hoort bij het image dat die casus serveert, en het portaal is het enige dat
# hem mag bereiken — hij luistert daarom alleen op localhost.
#
# Zonder CLAUDE_CODE_OAUTH_TOKEN start er geen assistent. Dat is geen fout: het
# portaal antwoordt dan 503 op /api en de app verbergt zijn paneel via de
# health-probe die hij toch al doet.
set -eu

if [ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}${ANTHROPIC_API_KEY:-}" ]; then
  poort=3600
  for casus in ${POC_ASSISTENT_CASUSSEN:-terugbetaalregimes nieuwkomersbekostiging}; do
    slug_env=$(echo "$casus" | tr 'a-z-' 'A-Z_')
    echo "beleidsassistent $casus op 127.0.0.1:$poort"
    POC_CASUS="$casus" \
    POC_CASUS_DIR="/app/casus/$casus" \
    POC_WASM_DIR="/app/static/$casus/wasm/pkg" \
    POC_VARIANT_OPSLAG=0 \
    PORT="$poort" \
      node /app/assistent/index.js &
    export "POC_ASSISTENT_${slug_env}=http://127.0.0.1:$poort"
    poort=$((poort + 1))
  done
else
  echo "geen CLAUDE_CODE_OAUTH_TOKEN: de beleidsassistent blijft uit"
fi

exec regelrecht-poc-portal
