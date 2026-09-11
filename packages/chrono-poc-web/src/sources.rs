//! Waar de wereld en het corpus vandaan komen, en hoe ze bij het starten op
//! schijf belanden.
//!
//! Twee bronnen, dezelfde grammatica:
//!
//! ```text
//! local:<pad>
//! github:<owner>/<repo>@<ref>:<pad>
//! ```
//!
//! De reden dat dit env-configuratie is en geen regel in `corpus-registry.yaml`:
//! het corpus van een casus kan privé zijn terwijl deze code en het image dat
//! eruit rolt publiek zijn. Een bron in de registry zou de naam van die repo in
//! een publiek bestand zetten, en dat is precies het gegeven dat er niet in
//! hoort. Wat er wél gedeeld wordt met de rest van de workspace is de
//! **tokenconventie**: `CORPUS_AUTH_<SLUG>_TOKEN`, opgelost door
//! [`regelrecht_corpus::auth::CredentialResolver`], zodat een operator voor deze
//! app niets nieuws hoeft te leren.
//!
//! Het ophalen gebeurt **één keer, bij het starten**, en falen betekent dat het
//! proces stopt. Een server die opkomt zonder wereld zou per verzoek dezelfde
//! fout geven en zou op een healthcheck groen staan; dat is de duurste vorm van
//! "hij draait".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regelrecht_corpus::auth::{token_env_name, CredentialResolver, TokenContext};
use regelrecht_github::GithubClient;
use regelrecht_simulator::WorldDefinition;
use tempfile::TempDir;

/// Waar één ding (het wereldbestand, de regelingenmap) vandaan komt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Een pad op de schijf van dit proces.
    Local(PathBuf),
    /// Een pad in een GitHub-repo, op een ref.
    Github {
        /// `owner/repo`.
        repo: String,
        /// Branch, tag of sha.
        git_ref: String,
        /// Het pad ín de repo.
        path: String,
    },
}

impl Source {
    /// Lees een bron uit haar env-waarde.
    ///
    /// Faalt met een melding die de hele grammatica noemt: dit wordt gelezen
    /// door iemand die een ZAD-variabele intypt, en "ongeldige bron" zonder de
    /// vorm erbij kost die persoon een deploy.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if let Some(path) = spec.strip_prefix("local:") {
            if path.is_empty() {
                return Err("'local:' mist een pad. Verwacht 'local:<pad>' of \
                     'github:<owner>/<repo>@<ref>:<pad>'."
                    .to_string());
            }
            return Ok(Self::Local(PathBuf::from(path)));
        }
        if let Some(rest) = spec.strip_prefix("github:") {
            return Self::parse_github(rest).map_err(|reason| {
                format!(
                    "'{spec}' is geen geldige github-bron: {reason}. \
                     Verwacht 'github:<owner>/<repo>@<ref>:<pad>'."
                )
            });
        }
        Err(format!(
            "'{spec}' begint niet met 'local:' of 'github:'. Verwacht \
             'local:<pad>' of 'github:<owner>/<repo>@<ref>:<pad>'."
        ))
    }

    /// `<owner>/<repo>@<ref>:<pad>`.
    ///
    /// Gesplitst op de **eerste** `@` en daarna op de **eerste** `:`. Een ref mag
    /// een `/` bevatten (`release/2027`) en een pad ook; geen van beide mag een
    /// `:`, en dat is de enige beperking die deze vorm oplegt.
    fn parse_github(rest: &str) -> Result<Self, String> {
        let (repo, after) = rest.split_once('@').ok_or("er staat geen '@<ref>' in")?;
        let (git_ref, path) = after.split_once(':').ok_or("er staat geen ':<pad>' in")?;
        let segments: Vec<&str> = repo.split('/').collect();
        if segments.len() != 2 || segments.iter().any(|s| s.is_empty()) {
            return Err(format!("'{repo}' is geen '<owner>/<repo>'"));
        }
        if git_ref.is_empty() {
            return Err("de ref is leeg".to_string());
        }
        if path.is_empty() {
            return Err("het pad is leeg".to_string());
        }
        Ok(Self::Github {
            repo: repo.to_string(),
            git_ref: git_ref.to_string(),
            path: path.to_string(),
        })
    }

    /// De sleutel waaronder het token van deze bron opgezocht wordt: de
    /// reponaam, tenzij de configuratie iets anders noemt.
    ///
    /// De reponaam als standaard, zodat een operator die twee privérepo's
    /// aanspreekt niet twee bronnen op één token ziet uitkomen. De override
    /// bestaat omdat een reponaam een lange env-variabele oplevert en een
    /// deployment vaak al een kortere naam voor dezelfde repo heeft.
    fn auth_ref(&self, override_ref: Option<&str>) -> Option<String> {
        match self {
            Self::Local(_) => None,
            Self::Github { repo, .. } => Some(match override_ref {
                Some(explicit) => explicit.to_string(),
                None => repo.rsplit('/').next().unwrap_or(repo).to_string(),
            }),
        }
    }
}

/// Wat er na het ophalen klaarstaat: de wereld-definitie en de regelingenmap.
#[derive(Debug)]
pub struct Resolved {
    /// Het gelezen wereldbestand. Elke sessie tuigt hier haar eigen wereld uit
    /// op, en `POST /api/reset` doet dat opnieuw.
    pub definition: WorldDefinition,
    /// De wortel waaronder de cellen hun regelingen vinden.
    pub regulation_root: PathBuf,
    /// De tijdelijke mappen met opgehaalde repo's.
    ///
    /// **Vasthouden zolang het proces leeft.** Een `TempDir` ruimt zichzelf op
    /// bij `Drop`, dus dit is wat de regelingen op schijf houdt — en wat ze
    /// opruimt als het proces netjes afsluit. Wie dit veld laat vallen, trekt de
    /// regelingen onder de cellen vandaan.
    pub fetched: Vec<TempDir>,
}

/// Haal wereld en corpus op en lees het wereldbestand.
///
/// `corpus` afwezig betekent: de simulator kiest zelf zijn wortel
/// ([`regelrecht_simulator::regulation_root`], dus `REGULATION_PATH` of het
/// corpus in deze checkout). Dat is de lokale modus, waarin een ontwikkelaar
/// niets hoeft te configureren om de publieke wereld te zien.
///
/// Twee bronnen in dezelfde repo op dezelfde ref worden **één keer** gehaald.
/// Dat is niet alleen sneller: het is ook het enige dat consistent is — een
/// wereldbestand en de regelingen waarop het rust, horen uit dezelfde
/// momentopname te komen.
pub async fn resolve(
    world: &Source,
    corpus: Option<&Source>,
    auth_ref_override: Option<&str>,
) -> Result<Resolved, String> {
    let mut fetcher = Fetcher::new(auth_ref_override);

    let world_path = fetcher.locate(world).await?;
    let definition = WorldDefinition::load(&world_path).map_err(|e| {
        format!(
            "kon het wereldbestand '{}' niet lezen: {e}",
            world_path.display()
        )
    })?;

    let regulation_root = match corpus {
        Some(source) => fetcher.locate(source).await?,
        None => regelrecht_simulator::regulation_root(),
    };
    if !regulation_root.is_dir() {
        return Err(format!(
            "de regelingenmap '{}' bestaat niet. Zet CHRONO_POC_CORPUS_SOURCE \
             of REGULATION_PATH naar een map met regelingen.",
            regulation_root.display()
        ));
    }

    Ok(Resolved {
        definition,
        regulation_root,
        fetched: fetcher.into_dirs(),
    })
}

/// Haalt repo's op en houdt bij wat er al gehaald is.
struct Fetcher<'a> {
    auth_ref_override: Option<&'a str>,
    /// `(repo, ref)` → de map waarin die momentopname staat.
    fetched: BTreeMap<(String, String), PathBuf>,
    dirs: Vec<TempDir>,
}

impl<'a> Fetcher<'a> {
    fn new(auth_ref_override: Option<&'a str>) -> Self {
        Self {
            auth_ref_override,
            fetched: BTreeMap::new(),
            dirs: Vec::new(),
        }
    }

    fn into_dirs(self) -> Vec<TempDir> {
        self.dirs
    }

    /// Het pad op schijf waar deze bron naar wijst, desnoods na ophalen.
    async fn locate(&mut self, source: &Source) -> Result<PathBuf, String> {
        match source {
            Source::Local(path) => {
                if !path.exists() {
                    return Err(format!("'{}' bestaat niet", path.display()));
                }
                Ok(path.clone())
            }
            Source::Github {
                repo,
                git_ref,
                path,
            } => {
                let key = (repo.clone(), git_ref.clone());
                if let Some(root) = self.fetched.get(&key) {
                    // Dezelfde toets als na een verse haal: een tweede bron uit
                    // een al opgehaalde momentopname hoort dezelfde melding te
                    // krijgen als de eerste. Zonder dit zou een corpuspad dat er
                    // niet in staat als "de regelingenmap bestaat niet — zet
                    // CHRONO_POC_CORPUS_SOURCE" terugkomen, van een variabele die
                    // wél gezet is en alleen het verkeerde pad noemt.
                    return located_in_repo(root, path, repo, git_ref);
                }
                let token = resolve_token(source, self.auth_ref_override)?;
                let dir = tempfile::Builder::new()
                    .prefix("chrono-poc-")
                    .tempdir()
                    .map_err(|e| format!("kon geen tijdelijke map maken: {e}"))?;
                let client = GithubClient::new()
                    .map_err(|e| format!("kon geen GitHub-client maken: {e}"))?;
                tracing::info!(repo = %repo, git_ref = %git_ref, "wereldbron ophalen");
                regelrecht_corpus::github::fetch_archive_to_dir(
                    &client,
                    repo,
                    git_ref,
                    token.as_deref(),
                    dir.path(),
                )
                .await
                .map_err(|e| format!("kon {repo}@{git_ref} niet ophalen: {e}"))?;
                let root = dir.path().to_path_buf();
                self.dirs.push(dir);
                self.fetched.insert(key, root.clone());
                located_in_repo(&root, path, repo, git_ref)
            }
        }
    }
}

/// Het pad ín een opgehaalde momentopname, mits het er ook echt in staat.
fn located_in_repo(root: &Path, path: &str, repo: &str, git_ref: &str) -> Result<PathBuf, String> {
    let located = join_in_repo(root, path)?;
    if !located.exists() {
        return Err(format!(
            "'{path}' staat niet in {repo}@{git_ref} (wel opgehaald, maar dit pad bestaat er niet)"
        ));
    }
    Ok(located)
}

/// Voeg een repo-relatief pad samen met de map waarin de repo staat.
///
/// Een leidende `/` of een `./` hoort niet uit te maken; een `..` wel, en die
/// wordt geweigerd. Niet omdat een operator zijn eigen server zou aanvallen, maar
/// omdat `github:owner/repo@main:../etc/passwd` dan iets van de container zou
/// lezen en als "de wereld" zou aanbieden — en een bron die buiten zijn repo
/// wijst, is in elk geval een vergissing.
fn join_in_repo(root: &Path, path: &str) -> Result<PathBuf, String> {
    use std::path::Component;
    let mut joined = root.to_path_buf();
    for component in Path::new(path.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => joined.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("'{path}' wijst buiten de opgehaalde repo"))
            }
        }
    }
    Ok(joined)
}

/// Zoek het token voor een github-bron op, en zeg in het log welke variabele
/// bekeken is.
///
/// Strikt: de sleutel komt uit onze eigen configuratie en niet uit een verzoek,
/// maar een strikte opzoeking is wat voorkomt dat een gedeeld token van de
/// harvester-tijd naar een repo gaat die deze app aanwijst. Geen token is geen
/// fout — een publieke repo heeft er geen nodig; een privérepo faalt hierna op
/// de 404 van GitHub, met de naam van de variabele in dit logregel ernaast.
fn resolve_token(
    source: &Source,
    auth_ref_override: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(auth_ref) = source.auth_ref(auth_ref_override) else {
        return Ok(None);
    };
    let env_name = token_env_name(&auth_ref);
    let decision = CredentialResolver::new(None)
        .resolve(TokenContext::strict(&auth_ref))
        .map_err(|e| format!("kon het token voor '{auth_ref}' niet opzoeken: {e}"))?;
    match decision.token() {
        Some(_) => tracing::info!(auth_ref = %auth_ref, env = %env_name, "token gevonden"),
        None => tracing::warn!(
            auth_ref = %auth_ref,
            env = %env_name,
            "geen token gevonden; het ophalen gaat onauthenticated en faalt op een privérepo"
        ),
    }
    Ok(decision.into_token())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn een_lokale_bron_is_een_pad() {
        assert_eq!(
            Source::parse("local:packages/simulator/worlds/publieke_wereld.yaml"),
            Ok(Source::Local(PathBuf::from(
                "packages/simulator/worlds/publieke_wereld.yaml"
            )))
        );
    }

    #[test]
    fn een_github_bron_valt_uiteen_in_repo_ref_en_pad() {
        assert_eq!(
            Source::parse("github:MinBZK/regelrecht@main:packages/simulator/worlds/w.yaml"),
            Ok(Source::Github {
                repo: "MinBZK/regelrecht".to_string(),
                git_ref: "main".to_string(),
                path: "packages/simulator/worlds/w.yaml".to_string(),
            })
        );
    }

    /// Een ref met een `/` erin is gewoon een ref. Gesplitst op de eerste `@`
    /// en de eerste `:`, dus `release/2027` blijft heel.
    #[test]
    fn een_ref_mag_een_schuine_streep_hebben() {
        assert_eq!(
            Source::parse("github:owner/repo@release/2027:simulator/wereld.yaml"),
            Ok(Source::Github {
                repo: "owner/repo".to_string(),
                git_ref: "release/2027".to_string(),
                path: "simulator/wereld.yaml".to_string(),
            })
        );
    }

    /// Elke afwijzing noemt de hele vorm. Wie een ZAD-variabele intypt, hoort
    /// niet te hoeven raden welk stukje ontbrak.
    #[test]
    fn een_onvolledige_bron_wordt_geweigerd_met_de_vorm_erbij() {
        for spec in [
            "",
            "packages/simulator/worlds/w.yaml",
            "local:",
            "github:MinBZK/regelrecht:pad.yaml",
            "github:MinBZK/regelrecht@main",
            "github:regelrecht@main:pad.yaml",
            "github:MinBZK/regelrecht@:pad.yaml",
            "github:MinBZK/regelrecht@main:",
        ] {
            let err = Source::parse(spec).expect_err("'{spec}' moet geweigerd worden");
            assert!(
                err.contains("local:<pad>") || err.contains("github:<owner>/<repo>@<ref>:<pad>"),
                "de melding voor '{spec}' noemt de verwachte vorm niet: {err}"
            );
        }
    }

    /// De standaard-sleutel is de reponaam, en de override wint. Samen met
    /// `token_env_name` is dat de hele afspraak die een operator moet kennen.
    #[test]
    fn de_tokensleutel_komt_uit_de_reponaam_of_uit_de_override() {
        let source = Source::parse("github:MinBZK/regelrecht-corpus-demo@main:simulator/w.yaml")
            .expect("geldige bron");
        assert_eq!(
            source.auth_ref(None).as_deref(),
            Some("regelrecht-corpus-demo")
        );
        assert_eq!(
            token_env_name(
                &source
                    .auth_ref(None)
                    .expect("github-bron heeft een sleutel")
            ),
            "CORPUS_AUTH_REGELRECHT_CORPUS_DEMO_TOKEN"
        );
        assert_eq!(source.auth_ref(Some("kort")).as_deref(), Some("kort"));
    }

    /// Een lokale bron vraagt nooit om een token: er is geen repo om bij in te
    /// loggen, en een variabele die niets doet hoort niet in een logregel.
    #[test]
    fn een_lokale_bron_heeft_geen_tokensleutel() {
        assert_eq!(Source::Local(PathBuf::from("w.yaml")).auth_ref(None), None);
    }

    /// Het pad ín de repo blijft ín de repo. Een leidende `/` of `./` maakt niet
    /// uit, een `..` wel: die zou buiten de opgehaalde momentopname wijzen en daar
    /// iets van de container als "de wereld" aanbieden.
    #[test]
    fn een_pad_in_de_repo_blijft_in_de_repo() {
        let root = Path::new("/tmp/opgehaald");
        for path in [
            "simulator/w.yaml",
            "/simulator/w.yaml",
            "./simulator/w.yaml",
        ] {
            assert_eq!(
                join_in_repo(root, path).as_deref(),
                Ok(Path::new("/tmp/opgehaald/simulator/w.yaml")),
                "'{path}' hoort gewoon samengevoegd te worden"
            );
        }
        for path in ["../buiten.yaml", "simulator/../../buiten.yaml"] {
            let err = join_in_repo(root, path).expect_err("'{path}' moet geweigerd worden");
            assert!(err.contains("buiten de opgehaalde repo"), "{err}");
        }
    }

    /// De belofte uit de moduledocs: een bron die niet bestaat laat het opstarten
    /// falen met het pad erin, in plaats van een server die opkomt zonder wereld.
    #[tokio::test]
    async fn een_ontbrekend_wereldbestand_laat_het_opstarten_falen() {
        let err = resolve(
            &Source::Local(PathBuf::from("/bestaat/niet.yaml")),
            None,
            None,
        )
        .await
        .expect_err("zonder wereldbestand hoort het opstarten te falen");
        assert!(
            err.contains("/bestaat/niet.yaml"),
            "de melding noemt het pad niet: {err}"
        );
    }
}
