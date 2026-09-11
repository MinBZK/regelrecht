//! Eén wereld per browsersessie, in geheugen, met een eigen thread.
//!
//! ## Waarom een thread en niet een slot
//!
//! Een [`World`] is **niet `Send`**. Dat is geen ongeluk: een cel houdt haar
//! engine in een `RefCell`, en die engine kan een `Rc<dyn CellResolver>` dragen
//! voor de duur van een besluit. Beide zijn precies goed voor wat ze zijn — één
//! wereld is één samenhangend geheel dat nooit over threads verdeeld hoort te
//! worden — maar ze maken `DashMap<SessionId, World>` onmogelijk: axum wil zijn
//! state `Send + Sync`, en een `Mutex<World>` is alleen `Sync` als `World`
//! `Send` is.
//!
//! Dus draait elke wereld op haar **eigen thread**, die haar bezit, en gaat elke
//! aanraking als opdracht over een kanaal naar die thread. Wat er in de map staat
//! is de *greep* op die thread: een zender en het moment waarop hij het laatst
//! gebruikt is. Dat levert op wat de wereld-in-de-map ook opgeleverd zou hebben —
//! sessies raken elkaar niet, twee sessies zijn twee werelden — plus iets extra's:
//! twee sessies wachten ook niet op elkaar, want ze delen geen slot.
//!
//! De prijs is een thread per actieve sessie. Voor een PoC met een handvol
//! gelijktijdige lezers is dat goedkoper dan de alternatieven (een
//! single-thread-runtime waarop álle sessies elkaar in de weg zitten, of de
//! engine `Send` maken), en de TTL hieronder is wat de prijs begrenst.
//!
//! ## Lui, en met een houdbaarheidsdatum
//!
//! De wereld van een sessie wordt opgetuigd bij de **eerste** opdracht, niet bij
//! de eerste aanraking van een cookie: een crawler die `/health` opvraagt hoort
//! geen regelingen te laten inlezen. Hij wordt opgeruimd als er een uur niets
//! gebeurd is ([`WorldRegistry::sweep`]); de zender verdwijnt uit de map, het
//! kanaal sluit, de thread eindigt en de wereld valt weg. Er is geen database en
//! geen tweede replica: wie in een andere pod terechtkomt, begint opnieuw, en
//! dat is een bewuste grens van deze opstelling.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use regelrecht_simulator::{World, WorldDefinition};
use tokio::sync::{mpsc, oneshot};

use crate::error::ApiError;

/// Eén opdracht aan een wereld: doe dit, en stuur het antwoord zelf terug.
///
/// Eén type in plaats van een variant per route. Het antwoord zit in de
/// closure — die houdt zijn eigen `oneshot`-zender vast — dus de typering blijft
/// staan waar de route staat, en deze module hoeft geen enkele route te kennen.
/// `Err` draagt de melding van een wereld die niet opgetuigd kon worden.
type Task = Box<dyn for<'a> FnOnce(Result<&'a mut World, &'a str>) + Send + 'static>;

/// De werelden van alle sessies.
pub struct WorldRegistry {
    definition: Arc<WorldDefinition>,
    regulation_root: Arc<PathBuf>,
    ttl: Duration,
    sessions: DashMap<String, SessionWorld>,
}

/// De greep op één wereld: het kanaal naar haar thread, en wanneer hij het
/// laatst gebruikt is.
struct SessionWorld {
    tasks: mpsc::UnboundedSender<Task>,
    last_used: Instant,
}

impl WorldRegistry {
    /// Een register over één wereld-definitie, met een houdbaarheid per sessie.
    pub fn new(definition: WorldDefinition, regulation_root: PathBuf, ttl: Duration) -> Self {
        Self {
            definition: Arc::new(definition),
            regulation_root: Arc::new(regulation_root),
            ttl,
            sessions: DashMap::new(),
        }
    }

    /// Het wereldbestand waaruit elke sessie haar wereld optuigt.
    ///
    /// Voor wat er over een wereld te weten valt zonder haar aan te raken: welke
    /// cellen erin zitten en welke parameters een lexostatus documenteert. Dat
    /// hoort niet over de thread van een sessie te lopen — het is configuratie en
    /// geen stand — en het is bij álle sessies hetzelfde.
    pub fn definition(&self) -> &WorldDefinition {
        &self.definition
    }

    /// Toets of er uit deze definitie en deze regelingen werkelijk een wereld te
    /// bouwen is, en geef de melding van de simulator als dat niet zo is.
    ///
    /// Hoort bij het starten te gebeuren, vóór de listener opengaat. Zonder deze
    /// toets is "het wereldbestand is leesbaar" alles wat het opstarten
    /// vaststelt, en dat is minder dan het lijkt: een wereldbestand dat een
    /// regeling noemt die niet in de opgehaalde map staat — een corpusbron die
    /// één map te hoog wijst, een ref waarin die wet nog niet bestond — levert
    /// een proces op dat groen staat op `/health` en op elk verzoek dezelfde 500
    /// geeft. Precies de vorm van "hij draait" die deze crate belooft niet te
    /// hebben.
    ///
    /// De wereld wordt gebouwd en meteen weggegooid: hij is niet `Send`, dus hij
    /// verlaat deze thread niet, en wat hier getoetst wordt is dat hij te bouwen
    /// is. Het kost één keer het inlezen van de regelingen, op een blocking
    /// thread, op een moment dat er nog niemand wacht.
    pub async fn check_buildable(&self) -> Result<(), String> {
        let definition = Arc::clone(&self.definition);
        let regulation_root = Arc::clone(&self.regulation_root);
        tokio::task::spawn_blocking(move || {
            World::from_definition(&definition, &regulation_root)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("de toets op het wereldbestand liep vast: {e}"))?
    }

    /// Hoeveel sessies er nu een wereld hebben.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Heeft nog geen enkele sessie een wereld?
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Doe iets met de wereld van deze sessie, op haar eigen thread.
    ///
    /// De enige ingang. Elke route loopt hierlangs, dus er is geen tweede plek
    /// die een wereld aanraakt — en daarmee geen tweede plek die de `last_used`
    /// zou kunnen vergeten bij te werken of buiten de eigen thread om zou kunnen
    /// grijpen.
    pub async fn with_world<T, F>(&self, session: &str, work: F) -> Result<T, ApiError>
    where
        F: for<'a> FnOnce(&'a mut World) -> Result<T, ApiError> + Send + 'static,
        T: Send + 'static,
    {
        // De greep wordt hier gepakt en het slot van de shard valt aan het eind
        // van dit blok. Een `DashMap`-guard over een `await` heen vasthouden is
        // hoe je een deadlock bouwt die zich als een hangend verzoek voordoet.
        let tasks = self.handle(session);

        let (answer, wait) = oneshot::channel();
        let task: Task = Box::new(move |world| {
            let result = match world {
                Ok(world) => work(world),
                Err(reason) => Err(ApiError::internal(reason.to_string())),
            };
            // Geen ontvanger meer: de client is weggelopen terwijl de wereld nog
            // aan het werk was. Het werk is gedaan en de wereld is bijgewerkt;
            // er is niets om te melden.
            let _ = answer.send(result);
        });

        if tasks.send(task).is_err() {
            return Err(self.gone(session));
        }
        match wait.await {
            Ok(result) => result,
            // De thread is weggevallen terwijl hij aan het werk was — een paniek
            // in de simulator is de enige manier waarop dat kan. Wat er dan aan
            // die wereld nog waar is, weet niemand; hem weggooien is de enige
            // eerlijke uitkomst.
            Err(_) => Err(self.gone(session)),
        }
    }

    /// De thread van deze sessie leeft niet meer: gooi de dode greep weg zodat
    /// een volgend verzoek een verse wereld krijgt.
    ///
    /// Alleen als de zender ook echt gesloten is. Tussen het mislukken en dit
    /// moment kan een ander verzoek al een nieuwe wereld voor dezelfde sessie
    /// hebben opgetuigd, en die hoort hier niet met de dode weg te gaan.
    fn gone(&self, session: &str) -> ApiError {
        self.sessions
            .remove_if(session, |_, world| world.tasks.is_closed());
        ApiError::internal("de wereld van deze sessie is gestopt; probeer het opnieuw")
    }

    /// De zender naar de thread van deze sessie; start hem als hij er nog niet is.
    fn handle(&self, session: &str) -> mpsc::UnboundedSender<Task> {
        let mut entry = self
            .sessions
            .entry(session.to_string())
            .or_insert_with(|| SessionWorld {
                tasks: self.spawn(session),
                last_used: Instant::now(),
            });
        entry.last_used = Instant::now();
        entry.tasks.clone()
    }

    /// Start de thread die de wereld van deze sessie bezit.
    fn spawn(&self, session: &str) -> mpsc::UnboundedSender<Task> {
        let (tasks, mut queue) = mpsc::unbounded_channel::<Task>();
        let definition = Arc::clone(&self.definition);
        let regulation_root = Arc::clone(&self.regulation_root);
        let label = session.to_string();

        let started = std::thread::Builder::new()
            .name(format!("wereld-{label}"))
            .spawn(move || {
                // Hier, en niet bij het aanmaken van de greep: het inlezen van de
                // regelingen kost tijd, en die hoort op deze thread te vallen en
                // niet op de runtime die het verzoek behandelt.
                let mut world = match World::from_definition(&definition, &regulation_root) {
                    Ok(world) => Ok(world),
                    Err(e) => {
                        tracing::error!(error = %e, "kon de wereld van een sessie niet optuigen");
                        Err(format!("kon de wereld niet optuigen: {e}"))
                    }
                };
                // `blocking_recv` op een eigen thread, buiten elke runtime: dit
                // is de hele reden dat die thread bestaat.
                while let Some(task) = queue.blocking_recv() {
                    match &mut world {
                        Ok(world) => task(Ok(world)),
                        Err(reason) => task(Err(reason)),
                    }
                }
                tracing::debug!(session = %label, "wereld opgeruimd");
            });

        if let Err(e) = started {
            // Geen thread: elke opdracht loopt dan op een gesloten kanaal, en
            // `with_world` maakt daar een 500 van. Niet paniekeren om één
            // sessie — de rest van de server werkt.
            tracing::error!(error = %e, "kon geen thread voor een wereld starten");
        }
        tasks
    }

    /// Ruim de sessies op die langer dan de TTL niets gedaan hebben.
    ///
    /// Geeft terug hoeveel er weg zijn. Het weggooien van de greep sluit het
    /// kanaal, waarna de thread zijn lus verlaat en de wereld laat vallen — er
    /// is dus niets te sluiten dat hier gesloten moet worden.
    ///
    /// `saturating_sub` en geen `-`: tussen de twee metingen kan er een sessie
    /// bijgekomen zijn, en dan is het verschil negatief. Een getal voor een
    /// logregel hoort het proces niet om te leggen.
    pub fn sweep(&self) -> usize {
        let ttl = self.ttl;
        let before = self.sessions.len();
        self.sessions
            .retain(|_, world| world.last_used.elapsed() < ttl);
        before.saturating_sub(self.sessions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Een wereld-definitie die geen regelingen nodig heeft: één bron-cel, geen
    /// wetten. Genoeg om het gedrag van dit register te toetsen zonder er een
    /// corpus bij te halen — dat is wat de HTTP-tests doen.
    fn definition() -> WorldDefinition {
        WorldDefinition::from_yaml(
            r"
clock:
  start: 2024-01-01
cells:
  - id: brp
    laws: []
    chronicles:
      - stream: relaties
        key: bsn
        events:
          - name: relatie_gewijzigd
            intake: levering
            recording_actor: brp
            grondslag: eigen registratie
            op_moment: 2023-03-01
            fields:
              bsn: '999993653'
              partnerschap_type: GEEN
    lexostatus_definitions:
      - name: partnerschap
        inputs:
          - name: bsn
            type: string
        outputs:
          - partnerschap_type
        reduction:
          chronicle: relaties
          key: bsn
          latest: true
",
        )
        .expect("de testwereld moet te lezen zijn")
    }

    fn registry(ttl: Duration) -> WorldRegistry {
        WorldRegistry::new(definition(), PathBuf::from("/bestaat/niet"), ttl)
    }

    /// Twee sessies zijn twee werelden. Hier op het niveau van het register: wat
    /// de ene sessie vooruit zet, staat bij de andere nog op de startdatum.
    #[tokio::test]
    async fn twee_sessies_zijn_twee_werelden() {
        let registry = registry(Duration::from_secs(3600));

        let advanced = registry
            .with_world("een", |world| {
                world.advance(chrono::NaiveDate::from_ymd_opt(2024, 6, 1).expect("datum"))?;
                Ok(world.now())
            })
            .await
            .expect("de wereld van 'een' moet vooruit kunnen");
        let untouched = registry
            .with_world("twee", |world| Ok(world.now()))
            .await
            .expect("de wereld van 'twee' moet opgetuigd worden");

        assert_eq!(advanced.to_string(), "2024-06-01");
        assert_eq!(untouched.to_string(), "2024-01-01");
        assert_eq!(registry.len(), 2);
    }

    /// Dezelfde sessie krijgt dezelfde wereld: wat er in het ene verzoek
    /// gebeurde, staat er in het volgende nog.
    #[tokio::test]
    async fn dezelfde_sessie_houdt_haar_wereld() {
        let registry = registry(Duration::from_secs(3600));

        registry
            .with_world("een", |world| {
                world.advance(chrono::NaiveDate::from_ymd_opt(2024, 6, 1).expect("datum"))?;
                Ok(())
            })
            .await
            .expect("vooruit");
        let clock = registry
            .with_world("een", |world| Ok(world.now()))
            .await
            .expect("beeld");

        assert_eq!(clock.to_string(), "2024-06-01");
        assert_eq!(registry.len(), 1);
    }

    /// Een sessie die niets meer doet, verdwijnt. Met een TTL van niets is elke
    /// sessie meteen verlopen, wat de ruimer toetsbaar maakt zonder een uur te
    /// wachten — en wat aantoont dat de wereld daarna vers opgetuigd wordt.
    #[tokio::test]
    async fn een_stille_sessie_wordt_opgeruimd() {
        let registry = registry(Duration::ZERO);

        registry
            .with_world("een", |world| {
                world.advance(chrono::NaiveDate::from_ymd_opt(2024, 6, 1).expect("datum"))?;
                Ok(())
            })
            .await
            .expect("vooruit");
        assert_eq!(registry.sweep(), 1, "de sessie hoort opgeruimd te zijn");
        assert!(registry.is_empty());

        let clock = registry
            .with_world("een", |world| Ok(world.now()))
            .await
            .expect("dezelfde sleutel krijgt een verse wereld");
        assert_eq!(clock.to_string(), "2024-01-01");
    }

    /// Een definitie die een regeling noemt die er niet is: leesbaar als YAML,
    /// maar er valt geen wereld uit te bouwen.
    fn unbuildable() -> WorldDefinition {
        WorldDefinition::from_yaml(
            r"
clock:
  start: 2024-01-01
cells:
  - id: toeslagen
    laws:
      - wet_die_niet_bestaat
",
        )
        .expect("de definitie zelf is leesbaar")
    }

    /// De toets bij het starten: een wereldbestand dat een regeling noemt die
    /// niet in de regelingenmap staat, hoort het proces te laten stoppen met de
    /// naam van die regeling erin — en niet een server op te leveren die groen
    /// staat op `/health` en op elk verzoek dezelfde 500 geeft.
    #[tokio::test]
    async fn een_onbouwbare_wereld_wordt_bij_het_starten_gezien() {
        let registry = WorldRegistry::new(
            unbuildable(),
            PathBuf::from("/bestaat/niet"),
            Duration::from_secs(60),
        );

        let err = registry
            .check_buildable()
            .await
            .expect_err("een wereld zonder regelingen hoort bij het starten te falen");
        assert!(
            err.contains("wet_die_niet_bestaat"),
            "de melding noemt de regeling niet: {err}"
        );
        assert!(
            registry.is_empty(),
            "de toets hoort geen sessie achter te laten"
        );
    }

    /// En de andere kant: een wereld die wél te bouwen is, laat het opstarten
    /// doorlopen. Zonder deze helft zou een toets die altijd faalt ook slagen.
    #[tokio::test]
    async fn een_bouwbare_wereld_komt_door_de_toets() {
        registry(Duration::from_secs(60))
            .check_buildable()
            .await
            .expect("de testwereld heeft geen regelingen nodig en hoort bouwbaar te zijn");
    }

    /// Een wereld die niet opgetuigd kan worden — hier: een regelingenmap die
    /// niet bestaat, terwijl de cel wetten laadt — komt als 500 terug met de
    /// melding van de simulator erin, en niet als een hangend verzoek.
    #[tokio::test]
    async fn een_wereld_die_niet_opkomt_geeft_een_leesbare_fout() {
        let registry = WorldRegistry::new(
            unbuildable(),
            PathBuf::from("/bestaat/niet"),
            Duration::from_secs(60),
        );

        let err = registry
            .with_world("een", |world| Ok(world.now()))
            .await
            .expect_err("een wereld zonder regelingen hoort te falen");

        assert_eq!(err.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            err.message().contains("wet_die_niet_bestaat"),
            "de melding noemt de regeling niet: {}",
            err.message()
        );
    }
}
