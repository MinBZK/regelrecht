-- Favorieten horen bij de plek waar je werkt.
--
-- Ze waren globaal per gebruiker, dus in elk traject stond precies dezelfde
-- lijst, ook als die niets met dat traject te maken had. Een favoriet is "de
-- wet waar ik hier steeds naar terugga", en "hier" is het traject.
--
-- NULL blijft de Corpus-juris-set: wat je sterrt terwijl je bladert, los van
-- welk traject dan ook. Dat is meteen wat de bestaande rijen zijn, dus er is
-- geen datamigratie nodig.
ALTER TABLE user_favorites
    ADD COLUMN traject_id UUID REFERENCES trajects(id) ON DELETE CASCADE;

-- Een primary key mag geen NULL bevatten, dus de uniciteit verhuist naar twee
-- partiële indexen: één voor de Corpus-juris-set en één per traject.
ALTER TABLE user_favorites DROP CONSTRAINT user_favorites_pkey;

CREATE UNIQUE INDEX user_favorites_corpus_uniq
    ON user_favorites (person_sub, law_id)
    WHERE traject_id IS NULL;

CREATE UNIQUE INDEX user_favorites_traject_uniq
    ON user_favorites (person_sub, traject_id, law_id)
    WHERE traject_id IS NOT NULL;

-- Het traject verwijderen neemt zijn favorieten mee (ON DELETE CASCADE); deze
-- index is wat die cascade en de per-traject-lijst snel houdt.
CREATE INDEX idx_user_favorites_traject
    ON user_favorites (traject_id)
    WHERE traject_id IS NOT NULL;
