//! De wereld: de cellen, de logische klok en de triggers die onderweg afgaan.
//!
//! Tijd is een eigenschap van de wereld en niet van een vraag. Eén logische
//! klok ([`NaiveDate`], nooit de wandklok) staat op een moment; [`World::advance`]
//! zet haar vooruit en loopt daarbij de triggers af die tussen het oude en het
//! nieuwe moment vallen, in datumvolgorde. Een trigger **voegt toe** en wijzigt
//! nooit een bestaand gram, dus het beeld van een eerder moment verandert niet
//! doordat de wereld verder loopt.
//!
//! Waarom in de wereld en niet in de cel: een cel kent alleen de momenten die
//! haar aangereikt worden. Zij houdt geen klok, precies zoals ze geen sleutels
//! en geen bevoegdheid houdt (RFC-022 §2).
//!
//! In deze versie bestaat één trigger-soort: een [`Fixture`] met een `at`-datum
//! die bij het passeren wordt vastgelegd. De lus is generiek — een gesorteerde
//! lijst van `(datum, trigger)` — zodat vervallende verplichtingen en gemiste
//! termijnen er later naast passen zonder dat de klok verandert.

use crate::cell::{Cell, CellConfig, ChronicleEvent, Intake, Lexostatus};
use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

/// De logische klok van een wereld, zoals het wereldbestand haar opgeeft.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// Het moment waarop de wereld begint. Verplicht: een wereld zonder
    /// startmoment zou op de wandklok moeten terugvallen, en dan is een run
    /// morgen een andere run.
    pub start: NaiveDate,
}

/// Eén startstand-gebeurtenis: wat er op welk moment wordt vastgelegd.
///
/// Een startstand is data. Fixtures op of vóór het startmoment van de klok staan
/// bij het optuigen al in de kroniek — de grens is inclusief, want een reductie
/// op het startmoment ziet wat op dat moment gebeurde. Latere fixtures zijn
/// triggers die afgaan wanneer [`World::advance`] hun datum passeert.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// Het moment waarop deze vastlegging gebeurt.
    pub at: NaiveDate,
    /// Wat er vastgelegd wordt, en bij wie.
    pub record: Recording,
}

/// Een vastlegging in de kroniek van één cel: de executogram-vorm van
/// RFC-022 §1.3, plus bij wie het gram landt.
///
/// `recording_actor` staat er niet bij: dat is [`Self::cell`]. Een kroniek
/// houdt alleen de eigen vastleggingen van de cel, dus die twee kunnen niet
/// uiteenlopen en hoeven niet twee keer opgeschreven.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// De cel die vastlegt, en in wiens kroniek het gram landt.
    pub cell: String,
    /// De kroniekstroom binnen die cel.
    pub chronicle: String,
    /// Wat er gebeurde.
    pub name: String,
    /// Het kanaal waarlangs het feit de cel bereikte.
    pub intake: Intake,
    /// Op welke grondslag dit feit vastgelegd wordt; mag leeg.
    #[serde(default)]
    pub grondslag: String,
    /// De vastgelegde velden.
    pub fields: BTreeMap<String, Value>,
}

impl Recording {
    /// Het executogram zoals het in de kroniek belandt, op het moment dat de
    /// klok passeert.
    fn event(&self, op_moment: NaiveDate) -> ChronicleEvent {
        ChronicleEvent {
            name: self.name.clone(),
            intake: self.intake,
            recording_actor: self.cell.clone(),
            grondslag: self.grondslag.clone(),
            op_moment,
            fields: self.fields.clone(),
        }
    }
}

/// Wat er gebeurt als de klok een moment passeert.
///
/// Eén soort in deze versie. De lus die ze afloopt is generiek, dus een soort
/// erbij is een variant erbij en geen andere klok.
#[derive(Debug, Clone)]
enum Trigger {
    /// Er wordt een executogram vastgelegd.
    Record(Recording),
}

/// Eén gesimuleerde wereld: cellen, een klok, en wat er nog moet gebeuren.
///
/// De wereld bezit de cellen en is de enige die hun vastleg-pad kan aanroepen.
/// Naar buiten is [`World::reduce`] de weg naar een cel, met de klok als grens:
/// een moment ná de klok is een fout en geen voorspelling.
#[derive(Debug)]
pub struct World {
    /// De cellen, op id.
    cells: BTreeMap<String, Cell>,
    /// Waar de logische klok staat.
    clock: NaiveDate,
    /// Wat er nog moet gebeuren, oplopend op datum. Bij een gelijke datum
    /// beslist de volgorde waarin de triggers zijn opgegeven; de sortering is
    /// stabiel, dus dat is een vastgelegde eigenschap en geen toeval.
    ///
    /// Oplopend is een **invariant**, niet een toestand: [`World::fire_due`]
    /// leest alleen de kop. Wie hier later iets bij zet — een trigger die een
    /// volgende trigger inplant — moet dat op datumpositie doen en niet
    /// achteraan, anders gaat wat erbij komt te laat af of helemaal niet.
    pending: VecDeque<(NaiveDate, Trigger)>,
}

impl World {
    /// Tuig een wereld op: bouw de cellen, zet de klok op haar startmoment en
    /// leg vast wat vóór dat moment al gebeurd was.
    ///
    /// Fixtures met een datum tot en met het startmoment landen hier meteen in
    /// de kroniek: de klok staat op `start`, dus wat toen al gebeurd was, is
    /// gebeurd. De rest blijft staan tot [`World::advance`] eraan komt.
    ///
    /// Elke fixture wordt hier getoetst zoals ze bij het vastleggen getoetst zou
    /// worden — onbekende cel, onbekende stroom, ontbrekend sleutelveld — ook als
    /// haar datum nog jaren weg is. Een typfout in een startstand hoort niet
    /// halverwege een tijdlijn op te duiken, en of dat gebeurt mag niet afhangen
    /// van hoe ver die datum weg ligt.
    pub fn new(
        configs: &[CellConfig],
        clock: Clock,
        fixtures: &[Fixture],
        regulation_root: &Path,
    ) -> Result<Self> {
        let mut cells: BTreeMap<String, Cell> = BTreeMap::new();
        for config in configs {
            if cells.contains_key(&config.id) {
                return Err(SimulatorError::DuplicateCell {
                    cell: config.id.clone(),
                });
            }
            cells.insert(
                config.id.clone(),
                Cell::from_config(config, regulation_root)?,
            );
        }

        for fixture in fixtures {
            let target = &fixture.record;
            let cell = cells
                .get(&target.cell)
                .ok_or_else(|| SimulatorError::UnknownCell {
                    cell: target.cell.clone(),
                })?;
            cell.check_recording(&target.chronicle, &target.event(fixture.at))?;
        }

        let mut pending: Vec<(NaiveDate, Trigger)> = fixtures
            .iter()
            .map(|fixture| (fixture.at, Trigger::Record(fixture.record.clone())))
            .collect();
        pending.sort_by_key(|(at, _)| *at);

        let mut world = Self {
            cells,
            clock: clock.start,
            pending: pending.into(),
        };
        world.fire_due(clock.start)?;
        Ok(world)
    }

    /// Waar de logische klok staat.
    pub fn now(&self) -> NaiveDate {
        self.clock
    }

    /// De cellen van deze wereld, geleend en niet in bezit.
    ///
    /// `pub(crate)`, niet publiek: dit is geen tweede weg voor een consument om
    /// bij een cel te komen (dat blijft [`World::reduce`]), maar de ingang die
    /// `InProcessTransport` nodig heeft om een vraag over een celgrens te
    /// zetten. Het transport houdt de cellen zelf ook geleend — zie
    /// `transport.rs` — dus dit voegt geen tweede eigenaar toe.
    pub(crate) fn cells(&self) -> &BTreeMap<String, Cell> {
        &self.cells
    }

    /// Zet de klok vooruit naar `tot` en laat onderweg elke trigger afgaan.
    ///
    /// De triggers gaan af in datumvolgorde, en tijdens een trigger staat de
    /// klok op het moment van die trigger — een vastlegging krijgt dus haar
    /// eigen datum als `op_moment`, niet de eindstand. Achteruit loopt de klok
    /// niet: wie het beeld van een eerder moment wil, vraagt dat op met
    /// `op_moment` in [`World::reduce`].
    pub fn advance(&mut self, tot: NaiveDate) -> Result<()> {
        if tot < self.clock {
            return Err(SimulatorError::ClockRunsBackwards {
                clock: self.clock.to_string(),
                to: tot.to_string(),
            });
        }
        self.fire_due(tot)?;
        self.clock = tot;
        Ok(())
    }

    /// Vraag een gepubliceerde lexostatus aan één cel, op één moment.
    ///
    /// `op_moment` mag niet ná de klok liggen. Wat na de klok gebeurt heeft nog
    /// niets vastgelegd, dus een antwoord "op" zo'n moment zou een voorspelling
    /// zijn die zich voordoet als een reductie. Ervóór mag wel, en levert het
    /// beeld van toen: de cel laat feiten die pas later vastlagen buiten
    /// beschouwing.
    ///
    /// De wereld combineert niets. Ze zoekt de cel op en geeft het antwoord
    /// door zoals de cel het gaf; synthese over cellen heen hoort bij een
    /// consument (RFC-022 §4.1).
    pub fn reduce(
        &self,
        cell: &str,
        lexostatus: &str,
        params: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Result<Lexostatus> {
        if op_moment > self.clock {
            return Err(SimulatorError::MomentAfterClock {
                cell: cell.to_string(),
                lexostatus: lexostatus.to_string(),
                op_moment: op_moment.to_string(),
                clock: self.clock.to_string(),
            });
        }
        self.cells
            .get(cell)
            .ok_or_else(|| SimulatorError::UnknownCell {
                cell: cell.to_string(),
            })?
            .reduce(lexostatus, params, op_moment)
    }

    /// Laat alles afgaan wat op of vóór `tot` valt, in datumvolgorde.
    ///
    /// De klok schuift mee naar het moment van de trigger die afgaat, zodat een
    /// trigger die zelf iets vastlegt dat op zijn eigen moment doet.
    ///
    /// Eén voor één van de kop af, en niet een hele kop in één keer weggehaald:
    /// valt een trigger om, dan blijven de triggers ná hem gewoon staan in
    /// plaats van met de fout te verdwijnen. Vandaag kan een trigger niet
    /// omvallen — [`World::new`] toetst elke fixture met dezelfde toets die het
    /// vastleggen gebruikt — maar dat is een eigenschap van de enige
    /// trigger-soort die er nu is, geen eigenschap van de lus.
    fn fire_due(&mut self, tot: NaiveDate) -> Result<()> {
        while let Some((at, trigger)) = self.next_due(tot) {
            self.clock = self.clock.max(at);
            match trigger {
                Trigger::Record(recording) => {
                    let event = recording.event(at);
                    let cell = self.cells.get_mut(&recording.cell).ok_or_else(|| {
                        SimulatorError::UnknownCell {
                            cell: recording.cell.clone(),
                        }
                    })?;
                    cell.record(&recording.chronicle, event)?;
                }
            }
        }
        Ok(())
    }

    /// De volgende trigger die op of vóór `tot` valt, van de kop van de lijst.
    ///
    /// De kop is genoeg omdat [`Self::pending`] oplopend is; is de eerste nog
    /// niet vervallen, dan is geen van de volgende dat.
    fn next_due(&mut self, tot: NaiveDate) -> Option<(NaiveDate, Trigger)> {
        let at = self.pending.front().map(|(at, _)| *at)?;
        if at > tot {
            return None;
        }
        self.pending.pop_front()
    }

    /// Hoeveel triggers nog niet afgegaan zijn.
    ///
    /// Voor het verslag van een run: een vastlegging die op geen enkel moment
    /// gevraagd wordt, gebeurt nooit, en dat hoort te zien te zijn in plaats van
    /// stil te blijven.
    pub fn pending_triggers(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::regulation_root;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
    }

    fn toeslagen() -> Vec<CellConfig> {
        let config = serde_yaml_ng::from_str(
            r"
id: toeslagen
laws:
  - algemene_wet_inkomensafhankelijke_regelingen
chronicles:
  - stream: relaties
    key: bsn
lexostatus_definitions:
  - name: toeslagpartnerschap
    inputs:
      - name: bsn
        type: string
    reduction:
      regulation: algemene_wet_inkomensafhankelijke_regelingen
      output: heeft_toeslagpartner
      parameters:
        bsn: $bsn
",
        )
        .unwrap_or_else(|e| panic!("testconfig moet parsen: {e}"));
        vec![config]
    }

    fn fixture(at: &str, chronicle: &str, partnerschap: &str) -> Fixture {
        Fixture {
            at: date(at),
            record: Recording {
                cell: "toeslagen".to_string(),
                chronicle: chronicle.to_string(),
                name: "relatie_gewijzigd".to_string(),
                intake: Intake::Levering,
                grondslag: "AWIR art. 3".to_string(),
                fields: BTreeMap::from([
                    ("bsn".to_string(), Value::String("999993653".to_string())),
                    (
                        "partnerschap_type".to_string(),
                        Value::String(partnerschap.to_string()),
                    ),
                ]),
            },
        }
    }

    fn world(clock_start: &str, fixtures: &[Fixture]) -> World {
        World::new(
            &toeslagen(),
            Clock {
                start: date(clock_start),
            },
            fixtures,
            &regulation_root(),
        )
        .unwrap_or_else(|e| panic!("wereld moet op te tuigen zijn: {e}"))
    }

    fn bsn() -> BTreeMap<String, Value> {
        BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))])
    }

    fn partner(world: &World, op_moment: &str) -> Value {
        world
            .reduce("toeslagen", "toeslagpartnerschap", &bsn(), date(op_moment))
            .unwrap_or_else(|e| panic!("de reductie op {op_moment} moet slagen: {e}"))
            .values()
            .unwrap_or_else(|| panic!("de reductie op {op_moment} hoort een antwoord te geven"))
            .get("heeft_toeslagpartner")
            .cloned()
            .unwrap_or_else(|| panic!("de reductie op {op_moment} hoort een antwoord te geven"))
    }

    #[test]
    fn fixture_van_voor_het_startmoment_staat_er_bij_het_optuigen_al() {
        let world = world(
            "2024-01-01",
            &[fixture("2023-01-01", "relaties", "HUWELIJK")],
        );
        assert_eq!(world.now(), date("2024-01-01"));
        assert_eq!(partner(&world, "2024-01-01"), Value::Bool(true));
    }

    #[test]
    fn een_latere_fixture_landt_pas_bij_advance_en_raakt_het_verleden_niet() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2023-01-01", "relaties", "HUWELIJK"),
                fixture("2024-07-01", "relaties", "GEEN"),
            ],
        );
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "een feit dat nog niet geland is, bestaat voor de cel niet"
        );

        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(partner(&world, "2025-01-01"), Value::Bool(false));
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "de vastlegging kreeg 2024-07-01 als moment, dus het beeld van \
             2024-01-01 blijft gelijk"
        );
    }

    /// Een trigger die nog moet afgaan mag de klok niet meenemen.
    ///
    /// Dit is de test die de lus vastpint in plaats van de tijdreductie. Elke
    /// andere assertie over de tijdlijn loopt via een reductie, en die filtert
    /// zelf al op `op_moment`, dus ze blijft ook groen als *elke* fixture
    /// meteen bij het optuigen wordt vastgelegd — de klok volledig negerend.
    /// Waar dat verschil wél zichtbaar wordt, is de stand van de klok: die
    /// bepaalt tot waar er gereduceerd mag worden, en een wereld die haar
    /// toekomst al heeft vastgelegd zou een antwoord geven op een moment
    /// waarover nog niets vaststaat.
    #[test]
    fn een_trigger_die_nog_moet_afgaan_zet_de_klok_niet_vooruit() {
        let world = world("2024-01-01", &[fixture("2024-07-01", "relaties", "GEEN")]);
        assert_eq!(
            world.now(),
            date("2024-01-01"),
            "de klok hoort op haar startmoment te staan, niet op dat van een \
             trigger die nog moet afgaan"
        );

        let err = world
            .reduce(
                "toeslagen",
                "toeslagpartnerschap",
                &bsn(),
                date("2024-07-01"),
            )
            .expect_err("het moment van een trigger die nog niet afging ligt ná de klok");
        assert!(
            matches!(err, SimulatorError::MomentAfterClock { .. }),
            "verwachtte MomentAfterClock, kreeg {err}"
        );
    }

    #[test]
    fn triggers_gaan_af_in_datumvolgorde_ongeacht_de_volgorde_in_het_bestand() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2024-09-01", "relaties", "GEEN"),
                fixture("2024-03-01", "relaties", "HUWELIJK"),
            ],
        );
        world
            .advance(date("2025-01-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            partner(&world, "2024-06-01"),
            Value::Bool(true),
            "tussen maart en september gold de vastlegging van maart"
        );
        assert_eq!(
            partner(&world, "2025-01-01"),
            Value::Bool(false),
            "na september geldt de vastlegging van september"
        );
    }

    #[test]
    fn een_moment_na_de_klok_wordt_geweigerd() {
        let world = world("2024-01-01", &[]);
        let err = world
            .reduce(
                "toeslagen",
                "toeslagpartnerschap",
                &bsn(),
                date("2024-06-01"),
            )
            .expect_err("een moment ná de klok hoort te falen");
        assert!(
            matches!(err, SimulatorError::MomentAfterClock { .. }),
            "verwachtte MomentAfterClock, kreeg {err}"
        );
    }

    #[test]
    fn de_klok_loopt_niet_terug() {
        let mut world = world("2024-06-01", &[]);
        let err = world
            .advance(date("2024-01-01"))
            .expect_err("achteruit lopen hoort te falen");
        assert!(
            matches!(err, SimulatorError::ClockRunsBackwards { .. }),
            "verwachtte ClockRunsBackwards, kreeg {err}"
        );
    }

    /// De executogram-vorm weigert wat ze niet kent, en dat geldt voor een
    /// veldnaam net zo goed als voor een kanaalnaam. Zonder die twee weigeringen
    /// staat er straks een vastlegging in een kroniek die iets anders zegt dan
    /// de auteur bedoelde: een grondslag die stil wegviel, of een kanaal
    /// waarvan niemand meer kan zeggen waarlangs het feit binnenkwam. Dat
    /// laatste is de hele reden dat `intake` een enum is en geen vrije tekst.
    #[test]
    fn een_onbekend_veld_of_kanaal_in_een_fixture_wordt_geweigerd() {
        let yaml = |grondslag: &str, intake: &str| {
            format!(
                r"
at: 2024-01-01
record:
  cell: toeslagen
  chronicle: relaties
  name: relatie_gewijzigd
  intake: {intake}
  {grondslag}: AWIR art. 3
  fields:
    bsn: '999993653'
"
            )
        };

        serde_yaml_ng::from_str::<Fixture>(&yaml("grondslag", "levering"))
            .unwrap_or_else(|e| panic!("een correcte fixture moet parsen: {e}"));
        assert!(
            serde_yaml_ng::from_str::<Fixture>(&yaml("grondlsag", "levering")).is_err(),
            "een typfout in een veldnaam hoort te falen; anders verdwijnt de grondslag stil"
        );
        assert!(
            serde_yaml_ng::from_str::<Fixture>(&yaml("grondslag", "leverng")).is_err(),
            "een typfout in een kanaalnaam hoort te falen"
        );
    }

    /// Twee triggers op dezelfde dag vallen op de tijdas niet uit elkaar; dan
    /// beslist de volgorde in het bestand, en de laatste wint. De sortering van
    /// de trigger-lijst is stabiel, dus dat is een vastgelegde eigenschap en
    /// geen toeval — zonder deze test zou een sortering die dat omgooit
    /// ongemerkt door kunnen.
    #[test]
    fn bij_een_gelijke_datum_beslist_de_volgorde_in_het_bestand() {
        let mut world = world(
            "2024-01-01",
            &[
                fixture("2024-06-01", "relaties", "HUWELIJK"),
                fixture("2024-06-01", "relaties", "GEEN"),
            ],
        );
        world
            .advance(date("2024-07-01"))
            .unwrap_or_else(|e| panic!("de klok moet vooruit kunnen: {e}"));

        assert_eq!(
            partner(&world, "2024-07-01"),
            Value::Bool(false),
            "de laatste van twee triggers op dezelfde dag hoort te winnen"
        );
    }

    #[test]
    fn een_fixture_op_het_startmoment_staat_er_bij_het_optuigen_al() {
        let world = world(
            "2024-01-01",
            &[fixture("2024-01-01", "relaties", "HUWELIJK")],
        );
        assert_eq!(
            partner(&world, "2024-01-01"),
            Value::Bool(true),
            "de grens is inclusief: wat op het startmoment gebeurde, is gebeurd, \
             en een reductie op dat moment hoort het te zien"
        );
    }

    #[test]
    fn een_fixture_naar_een_onbekende_stroom_faalt_bij_het_optuigen() {
        let err = World::new(
            &toeslagen(),
            Clock {
                start: date("2024-01-01"),
            },
            &[fixture("2030-01-01", "betalingen", "GEEN")],
            &regulation_root(),
        )
        .expect_err("een stroom die de cel niet houdt hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownStream { .. }),
            "verwachtte UnknownStream, kreeg {err}"
        );
    }

    #[test]
    fn een_fixture_naar_een_onbekende_cel_faalt_bij_het_optuigen() {
        let mut elders = fixture("2030-01-01", "relaties", "GEEN");
        elders.record.cell = "belastingdienst".to_string();
        let err = World::new(
            &toeslagen(),
            Clock {
                start: date("2024-01-01"),
            },
            &[elders],
            &regulation_root(),
        )
        .expect_err("een cel die de wereld niet kent hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownCell { .. }),
            "verwachtte UnknownCell, kreeg {err}"
        );
    }

    /// Een fixture die het sleutelveld van haar stroom mist, hoort bij het
    /// optuigen te falen en niet halverwege de tijdlijn — en dat mag niet
    /// afhangen van de datum. Stond die datum vóór het startmoment, dan viel de
    /// fout op omdat de vastlegging bij het optuigen afging; lag hij erna, dan
    /// kwam dezelfde typfout pas bij `advance` boven water.
    #[test]
    fn een_fixture_zonder_sleutelveld_faalt_bij_het_optuigen_ook_als_haar_datum_ver_weg_ligt() {
        for at in ["2023-01-01", "2030-01-01"] {
            let mut zonder_sleutel = fixture(at, "relaties", "GEEN");
            zonder_sleutel.record.fields.remove("bsn");
            let err = World::new(
                &toeslagen(),
                Clock {
                    start: date("2024-01-01"),
                },
                &[zonder_sleutel],
                &regulation_root(),
            )
            .expect_err("een vastlegging zonder sleutelveld hoort te falen");
            assert!(
                matches!(err, SimulatorError::ChronicleEventWithoutKey { .. }),
                "fixture van {at}: verwachtte ChronicleEventWithoutKey, kreeg {err}"
            );
        }
    }
}
