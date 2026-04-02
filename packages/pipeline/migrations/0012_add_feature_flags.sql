-- Feature flags for controlling UI panel visibility and other runtime settings.
-- Centralized table: any application can read/write flags via the pipeline crate.
CREATE TABLE feature_flags (
    key         TEXT PRIMARY KEY,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_feature_flags_updated_at
    BEFORE UPDATE ON feature_flags
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Seed editor panel flags
INSERT INTO feature_flags (key, enabled, description) VALUES
    ('panel.article_text', true, 'Wettekst (linker paneel)'),
    ('panel.scenario_form', true, 'Scenario formulier (midden paneel)'),
    ('panel.yaml_editor', true, 'YAML editor (midden paneel)'),
    ('panel.execution_trace', true, 'Resultaat (rechter paneel)'),
    ('panel.machine_readable', false, 'Machine readable weergave');
