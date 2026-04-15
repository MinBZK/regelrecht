CREATE TABLE user_favorites (
    person_sub  TEXT        NOT NULL,
    law_id      TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (person_sub, law_id)
);
