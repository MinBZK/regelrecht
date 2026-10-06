-- Eenmalige omzetting: regelrecht-corpus-NAPP is van MinBZK naar
-- NederlandseDigitaleDienst verhuisd.
--
-- GitHub stuurt de oude coördinaten door, maar die redirect breekt zodra
-- iemand in MinBZK opnieuw een repo met deze naam aanmaakt. Daarom wijzen
-- de bestaande traject-sources nu rechtstreeks naar de nieuwe eigenaar.
--
-- `auth_ref` gaat mee, zodat de token-lookup het env-var
-- `CORPUS_AUTH_NEDERLANDSEDIGITALEDIENST_REGELRECHT_CORPUS_NAPP_TOKEN` pakt,
-- dezelfde naam die `derive_auth_ref` voor een nieuw traject op deze repo
-- afleidt. Alleen rijen met de afgeleide MinBZK-waarde: een handmatig
-- gezette `auth_ref` blijft staan.
UPDATE traject_corpus_sources
SET auth_ref = 'nederlandsedigitaledienst-regelrecht-corpus-napp'
WHERE gh_owner = 'MinBZK'
  AND gh_repo = 'regelrecht-corpus-NAPP'
  AND auth_ref = 'minbzk-regelrecht-corpus-napp';

UPDATE traject_corpus_sources
SET gh_owner = 'NederlandseDigitaleDienst'
WHERE gh_owner = 'MinBZK'
  AND gh_repo = 'regelrecht-corpus-NAPP';
