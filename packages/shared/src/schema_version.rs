//! The schema version that newly written law YAML carries.
//!
//! Two production paths write corpus YAML — the harvester and the pipeline's
//! law-convert — and each used to pin its own `$schema` URL by hand. They drifted
//! (v0.5.4 against v0.5.6), so the two writers stamped two different contracts
//! into one corpus without anyone choosing that. Neither the golden tests (they
//! interpolate the writer's own constant) nor the CI guard (it only checks that
//! the version is *known*) could see the difference.
//!
//! One constant, one place, with `schema_version_matches_latest` asserting it
//! equals the `schema/latest` symlink so a bump cannot leave a writer behind.

/// Version stamped on newly written law YAML. Bump together with
/// `schema/latest`; the test below fails otherwise.
pub const CURRENT_SCHEMA_VERSION: &str = "v0.6.0";

/// `$schema` URL for newly written law YAML.
///
/// The immutable tag form (RFC-013), matching the convention in
/// `corpus/regulation/`, so written files never point at a moving target.
pub const SCHEMA_URL: &str = concat!(
    "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-",
    "v0.6.0",
    "/schema/",
    "v0.6.0",
    "/schema.json"
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Repo root, derived from this crate's manifest dir (`packages/shared`).
    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("resolve repo root")
    }

    /// The URL is written with the version spelled out twice (`concat!` needs
    /// literals), so this pins the three spellings to each other.
    #[test]
    fn schema_url_carries_the_current_version() {
        assert!(
            SCHEMA_URL.contains(&format!("schema-{CURRENT_SCHEMA_VERSION}/")),
            "SCHEMA_URL tag does not name {CURRENT_SCHEMA_VERSION}: {SCHEMA_URL}"
        );
        assert!(
            SCHEMA_URL.ends_with(&format!("/schema/{CURRENT_SCHEMA_VERSION}/schema.json")),
            "SCHEMA_URL path does not name {CURRENT_SCHEMA_VERSION}: {SCHEMA_URL}"
        );
    }

    /// A new schema version must not leave the writers stamping the old one.
    /// This is the check that was missing: the CI guard only asserts that a
    /// corpus file names a *known* version, never which one a writer stamps.
    #[test]
    fn schema_version_matches_latest() {
        let target = std::fs::read_link(repo_root().join("schema/latest"))
            .expect("schema/latest is a symlink");
        let latest = target
            .file_name()
            .expect("schema/latest points somewhere")
            .to_string_lossy()
            .to_string();
        assert_eq!(
            CURRENT_SCHEMA_VERSION, latest,
            "schema/latest is {latest} but new files would be stamped \
             {CURRENT_SCHEMA_VERSION}; bump CURRENT_SCHEMA_VERSION and SCHEMA_URL"
        );
    }

    /// And the version it names has to exist in this repo.
    #[test]
    fn current_schema_version_exists() {
        let dir = repo_root().join("schema").join(CURRENT_SCHEMA_VERSION);
        assert!(
            dir.join("schema.json").is_file(),
            "{} does not exist",
            dir.display()
        );
    }
}
