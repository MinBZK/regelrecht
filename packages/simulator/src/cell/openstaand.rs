//! Wat er van de eigen verplichtingen op een zaak nog openstaat.
//!
//! De derde reductievorm, en de eerste die twee eigen kronieken combineert. Naast
//! **verwacht** — wat een decretogram oplegt — en **gebeurd** — wat er in de
//! betalingenstroom ligt — bestaat het verschil tussen die twee, en dat verschil
//! is een lexostatus als elke andere: een reductie over eigen feiten, op een
//! moment, zonder tweede administratie eronder.
//!
//! **Nog steeds binnen de cel.** Beide stromen zijn van dezelfde cel (RFC-022
//! §4.1). Dat de vorm er twee leest, verandert daar niets aan; het is precies de
//! reden dat deze vraag *bij de cel* thuishoort en niet bij een consument die
//! twee antwoorden bij elkaar optelt.
//!
//! **Geen saldo.** Er wordt niets bijgehouden. Wat er openstond op een moment in
//! het verleden, blijft exact hetzelfde antwoord geven nadat er meer betaald is —
//! dezelfde eigenschap als bij de som over de betalingen, en om dezelfde reden:
//! het antwoord komt uit de grammen en niet uit een teller ernaast.
//!
//! **De klok komt niet altijd na.** Een termijn die vervallen is zonder dat er
//! betaald is, is geen fout in de opstelling maar een toestand die de wereld kan
//! voortbrengen — zie [`crate::CellConfig::betalingen_opgeschort`]. Zonder die
//! mogelijkheid zou deze reductie altijd nul opleveren en dus niets bewijzen.
//!
//! **Wat vervalt, staat er ook.** Een beschikking die in de plaats van een
//! eerdere komt, laat de termijnen vervallen die nog niet verstreken waren
//! (`vervangt_openstaande_termijnen`). Dat staat in geen enkel gram — een gram
//! verandert niet — maar het volgt wél uit de grammen: een later besluit over
//! dezelfde zaak dat het declareert, en een termijn die op dát moment nog moest
//! komen. Deze reductie leidt het daaruit af, want de vraag "wat staat er nog
//! open" mag geen bedrag noemen dat niemand meer hoeft te betalen.
//!
//! **Twee stages, één zaak.** De stroom `beschikkingen` draagt per zaak een gram
//! per stage (RFC-008, RFC-022 §1.2): het besluit zelf en zijn bekendmaking. Een
//! verplichting met `vanaf: bekendmaking` staat in het besluit-gram nog onder
//! `wacht_op_bekendmaking` en krijgt haar termijnen pas in het gram van de
//! bekendmaking. Deze reductie leest ze daarom allebei — het besluit voor wat er
//! opgelegd is, de bekendmaking voor wat daardoor is gaan lopen — en zolang die
//! bekendmaking er niet is, staat wat erop wacht in de lijst met haar eigen
//! stand. Weglaten zou een zaak waarover wel degelijk iets besloten is, laten
//! zien als een zaak waarover niets loopt.
//!
//! **Per richting, niet netto.** Een verplichting heeft een soort — een
//! `betaling` of, als het bedrag onder nul uitviel en de wet dat omkeert, een
//! `terugvordering` (Awb 4:57) — en die twee lopen de andere kant op. Ze bij
//! elkaar optellen levert een bedrag op dat geen van beide partijen verschuldigd
//! is; ze met een minteken verrekenen maakt er een saldo van, en een saldo is
//! precies wat deze reductie niet bijhoudt. De drie bedragen staan er daarom per
//! soort, en elke termijn in de lijst zegt van welke soort ze is.

use super::besluit::{
    wachtende_verplichtingen, ObligationKind, BEDRAG, BESCHIKKINGEN, BESLUIT, BESLUIT_CEL,
    BETALINGEN, OBLIGATIONS, SOORT, STAGE, STAGE_BEKENDMAKING, STAGE_BESLUIT,
    TERMIJNEN_VERVALLEN_DOOR, VOLGNUMMER, ZAAKKENMERK,
};
use super::chronicle::{self, ChronicleEvent};
use super::reductie::{GebruiktGram, Openstaandvorm, Reductie};
use super::{gram_tekst, Cell, LexostatusOutcome};
use crate::error::Result;
use crate::values::amount;
use chrono::NaiveDate;
use regelrecht_engine::Value;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Het veld van een termijn met de dag waarop ze vervalt.
const VERVALDATUM: &str = "vervaldatum";

/// Uitkomst met de termijnen waarover het gaat, elk met haar soort en stand.
pub(crate) const TERMIJNEN: &str = "termijnen";

/// De drie bedragen van één richting, onder hun gepubliceerde namen.
///
/// Voluit geschreven en niet uit de soort samengesteld: [`OUTPUTS`] moet vast
/// staan vóórdat er één vraag gesteld is, en een naam die pas bij het antwoord
/// ontstaat, kan het optuigen niet toetsen.
struct Richting {
    /// De soort verplichting waarover deze bedragen gaan.
    soort: ObligationKind,
    /// Wat er op dit moment van die soort verwacht werd.
    verwacht: &'static str,
    /// Wat daarvan in de eigen stroom ligt.
    betaald: &'static str,
    /// Het verschil tussen die twee.
    openstaand: &'static str,
}

/// Beide richtingen, altijd allebei.
///
/// Ook over een zaak zonder terugvordering: een uitkomst die er alleen soms is,
/// laat een lezer niet zien of er niets teruggevorderd wordt of dat hij het
/// antwoord maar half kreeg.
const RICHTINGEN: [Richting; 2] = [
    Richting {
        soort: ObligationKind::Betaling,
        verwacht: "betaling_verwacht",
        betaald: "betaling_betaald",
        openstaand: "betaling_openstaand",
    },
    Richting {
        soort: ObligationKind::Terugvordering,
        verwacht: "terugvordering_verwacht",
        betaald: "terugvordering_betaald",
        openstaand: "terugvordering_openstaand",
    },
];

/// Wat deze vorm publiceert; vast, want de vorm maakt ze zelf.
///
/// De bedragen en de lijst eronder horen bij elkaar: per richting is `verwacht`
/// min `betaald` gelijk aan `openstaand`, en `termijnen` laat zien waaruit die
/// bestaan. Een definitie die er één van zou mogen weglaten, zou een bedrag naar
/// buiten geven dat niet na te rekenen is.
pub(crate) const OUTPUTS: [&str; 7] = [
    RICHTINGEN[0].verwacht,
    RICHTINGEN[0].betaald,
    RICHTINGEN[0].openstaand,
    RICHTINGEN[1].verwacht,
    RICHTINGEN[1].betaald,
    RICHTINGEN[1].openstaand,
    TERMIJNEN,
];

/// De stand van één termijn op het gevraagde moment.
///
/// Een **open** verzameling, met opzet: er komen statussen bij zodra de wereld
/// meer met termijnen kan (een termijn die door een latere vaststelling vervalt,
/// een termijn die op een bekendmaking wacht). Wie hier leest, hoort een
/// onbekende status te kunnen tegenkomen en niet aan te nemen dat het er drie
/// zijn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Er ligt in de eigen betalingenstroom genoeg op deze termijn.
    Betaald,
    /// Ze vervalt vandaag en er ligt (nog) niets: openstaand, niet te laat.
    Open,
    /// Haar vervaldag is gepasseerd en er ligt niet genoeg.
    TeLaat,
    /// Ze heeft nog geen vervaldag: het besluit is nog niet bekendgemaakt.
    ///
    /// Opgelegd en toch niet verwacht. Een beschikking werkt pas door haar
    /// bekendmaking (Awb 3:40), dus tot dat moment is er niets te laat — maar er
    /// staat wél iets te gebeuren, en een lijst die daarover zwijgt laat een zaak
    /// er leeg uitzien terwijl er een verplichting in klaarligt.
    WachtOpBekendmaking,
    /// Een later besluit over dezelfde zaak kwam ervoor in de plaats.
    ///
    /// Ze staat nog in het gram — een gram verandert niet, en het voorschot
    /// beloofde wat het beloofde — maar er hoeft niet meer op betaald te worden.
    /// Haar als `te_laat` tonen zou een schuld noemen die de wet heeft laten
    /// vervallen; haar weglaten zou een belofte laten verdwijnen die er wél stond.
    Vervallen,
}

impl Status {
    /// Hoe deze stand in het antwoord heet.
    fn name(self) -> &'static str {
        match self {
            Self::Betaald => "betaald",
            Self::Open => "open",
            Self::TeLaat => "te_laat",
            Self::WachtOpBekendmaking => "wacht_op_bekendmaking",
            Self::Vervallen => "vervallen",
        }
    }
}

/// Eén regel van de lijst: een termijn, of een verplichting die nog wacht.
struct Termijn {
    /// De besluit-definitie waaruit ze volgt.
    besluit: String,
    /// Welke kant ze op loopt: een betaling of een terugvordering.
    soort: ObligationKind,
    /// Haar volgnummer binnen het schema van dat besluit.
    volgnummer: Value,
    /// De dag waarop ze vervalt; `None` zolang ze op de bekendmaking wacht.
    vervaldatum: Option<NaiveDate>,
    /// Het bedrag dat op die dag verwacht werd — of, bij een wachtende
    /// verplichting, het bedrag dat er in totaal uit gaat komen.
    bedrag: Decimal,
    /// Wat er in de eigen betalingenstroom op deze termijn ligt.
    betaald: Decimal,
    /// De betalingen die erop liggen, elk met wat ze van deze termijn dekte.
    ///
    /// Gedekt en niet betaald: wat er bovenop het termijnbedrag ligt, dekt
    /// niets, en een bijdrage die meer noemt dan er meetelt, laat de uitleg
    /// optellen tot een ander bedrag dan het antwoord.
    gedekt_door: Vec<(usize, Decimal)>,
    /// De plek van het gram waaruit ze komt.
    ///
    /// Nodig voor de uitleg: elk gelezen decretogram draagt als bijdrage de
    /// termijnen die eruit komen en meetellen.
    uit_gram: usize,
    /// Kwam er een later besluit over dezelfde zaak voor in de plaats?
    vervallen: bool,
}

impl Termijn {
    /// Telt deze regel mee in de bedragen van het antwoord?
    ///
    /// Alleen met een vervaldag, en alleen als ze niet vervallen is — of wél
    /// vervallen maar er toch iets op ligt. Wat nog op een bekendmaking wacht is
    /// opgelegd maar niet verwacht: er is geen dag waarop ze had moeten gebeuren,
    /// dus er valt niets te verwachten en niets te missen. Wat vervallen is,
    /// hoeft niemand meer te betalen; ligt er toch iets op, dan is dat betaald
    /// en telt het mee (zie [`Self::verwacht_bedrag`]).
    fn verwacht(&self) -> bool {
        self.vervaldatum.is_some() && (!self.vervallen || self.betaald > Decimal::ZERO)
    }

    /// Wat deze regel aan het verwachte bedrag bijdraagt.
    ///
    /// Het termijnbedrag, behalve bij een vervallen termijn: daar alleen wat er
    /// al op lag. Wat betaald is, is betaald — ook een deel — maar de rest hoeft
    /// niemand meer te betalen, dus die staat ook niet open.
    fn verwacht_bedrag(&self) -> Decimal {
        if !self.verwacht() {
            return Decimal::ZERO;
        }
        if self.vervallen {
            return self.betaald.min(self.bedrag);
        }
        self.bedrag
    }

    /// Reken een betaling op deze plek aan deze termijn toe.
    ///
    /// Ze dekt ten hoogste wat er nog niet lag: wat erbovenop komt, telt in
    /// `betaald` (de termijn ís dan betaald) maar niet als bijdrage.
    fn reken_toe(&mut self, plek: usize, bedrag: Decimal) {
        let ruimte = (self.bedrag - self.betaald).max(Decimal::ZERO);
        self.gedekt_door.push((plek, bedrag.min(ruimte)));
        self.betaald += bedrag;
    }

    /// Ligt er genoeg op deze termijn?
    fn volledig_betaald(&self) -> bool {
        self.betaald >= self.bedrag
    }

    /// Waarop de regels onderling geordend worden.
    ///
    /// Wat nog wacht, komt achteraan: het heeft geen dag om op te sorteren, en
    /// een lijst die met het ongedateerde begint, leest niet als een tijdlijn.
    /// Het volgnummer als getal en niet als waarde: het gram draagt het als
    /// getal, maar een startstand mag er iets anders in zetten, en een volgorde
    /// die daarop omvalt zou een lijst opleveren die van de invoer afhangt.
    fn sleutel(&self) -> (bool, Option<NaiveDate>, &str, Decimal) {
        (
            self.vervaldatum.is_none(),
            self.vervaldatum,
            self.besluit.as_str(),
            self.volgnummer.as_decimal().unwrap_or_default(),
        )
    }

    /// De stand van deze regel op het gevraagde moment.
    ///
    /// Betaald gaat vóór vervallen: wat er ligt, ligt er, ook als een later
    /// besluit de rest liet vervallen. Dat gebeurt als dat besluit met
    /// terugwerkende kracht genomen is — de klok was toen al langs deze termijn
    /// gekomen. Wat daarmee moet gebeuren is de verrekening in dat besluit en
    /// geen terugdraaiing.
    fn status(&self, op_moment: NaiveDate) -> Status {
        let Some(vervaldatum) = self.vervaldatum else {
            return if self.vervallen {
                Status::Vervallen
            } else {
                Status::WachtOpBekendmaking
            };
        };
        if self.volledig_betaald() {
            return Status::Betaald;
        }
        if self.vervallen {
            return Status::Vervallen;
        }
        if vervaldatum < op_moment {
            return Status::TeLaat;
        }
        Status::Open
    }

    /// Deze regel zoals ze in het antwoord komt te staan.
    fn as_value(&self, op_moment: NaiveDate) -> Value {
        Value::Object(BTreeMap::from([
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (VOLGNUMMER.to_string(), self.volgnummer.clone()),
            (
                SOORT.to_string(),
                Value::String(self.soort.name().to_string()),
            ),
            (
                VERVALDATUM.to_string(),
                // Uitdrukkelijk leeg en niet weggelaten: dat er geen vervaldag is,
                // is hier de mededeling. Een veld dat er soms niet is, laat een
                // lezer denken dat hij hem gemist heeft.
                self.vervaldatum
                    .map_or(Value::Null, |dag| Value::String(dag.to_string())),
            ),
            (BEDRAG.to_string(), amount(self.bedrag)),
            (
                "status".to_string(),
                Value::String(self.status(op_moment).name().to_string()),
            ),
        ]))
    }
}

/// Reduceer: wat legde deze cel op deze zaak op, en wat ligt daarvan betaald?
///
/// De volgorde is die van het antwoord zelf. Eerst de decretogrammen — per
/// besluitnaam het laatste, want een besluit dat een eerder besluit over dezelfde
/// zaak overschrijft, vervangt het en staat er niet naast. Dan hun termijnen, tot
/// en met het gevraagde moment. Dan de betalingen die daarbij horen. Wat overblijft
/// staat open.
///
/// Zonder decretogram over deze zaak is er niets vastgesteld, en dat is een
/// antwoord: een nul zou "hier staat niets open" niet kunnen onderscheiden van
/// "hier is geen zaak".
pub(crate) fn reduce(
    cell: &Cell,
    key: &str,
    key_value: &Value,
    op_moment: NaiveDate,
) -> Result<(LexostatusOutcome, Reductie)> {
    let vorm = Openstaandvorm {
        beschikkingen: BESCHIKKINGEN.to_string(),
        betalingen: BETALINGEN.to_string(),
        key: key.to_string(),
        key_value: key_value.clone(),
        op_moment,
    };
    let geen: BTreeMap<String, Value> = BTreeMap::new();

    let beschikkingen =
        cell.chronicles
            .recordings_with_place(BESCHIKKINGEN, key, key_value, &geen, op_moment);
    let laatste = laatste_per_besluit(beschikkingen);
    if laatste.iter().all(|zaak| zaak.besluit.is_none()) {
        return Ok((
            LexostatusOutcome::NotEstablished {
                reason: niets_opgelegd(key, key_value, op_moment),
            },
            Reductie::niets_vastgesteld(
                vorm,
                Vec::new(),
                cell.chronicles
                    .missed(BESCHIKKINGEN, key, key_value, &geen, op_moment),
            ),
        ));
    }

    // De betalingen van deze zaak, met hun plek: ze worden zo meteen per termijn
    // toegerekend, en alleen wat toegerekend raakt is gelezen.
    let betalingen = cell
        .chronicles
        .recordings_with_place(BETALINGEN, key, key_value, &geen, op_moment);

    let mut termijnen: Vec<Termijn> = Vec::new();
    let mut uit_de_beschikkingen: Vec<(usize, &ChronicleEvent)> = Vec::new();
    for zaak in laatste {
        // Het gram van het besluit zonder dat van zijn bekendmaking is een zaak
        // die hier niet bestaat: een bekendmaking wijst altijd een besluit aan.
        let Some((plek, gram)) = zaak.besluit else {
            continue;
        };
        let besluit = &zaak.naam;
        // Het eerste besluit over deze zaak ná een gram dat in de plaats kwam
        // van wat dat gram beloofde — dezelfde regel waarmee de bekendmaking
        // besluit of er nog iets gaat lopen — en dan alleen als het op het
        // gevraagde moment al genomen was: een antwoord over het verleden hoort
        // niet te veranderen door wat er later besloten werd.
        let zaakkenmerk = gram_tekst(gram, ZAAKKENMERK);
        let vervangen_op = |na: usize| -> Result<Option<NaiveDate>> {
            Ok(cell
                .vervangen_na(na, zaakkenmerk)?
                .map(|vervanger| vervanger.op_moment)
                .filter(|moment| *moment <= op_moment))
        };
        let na_het_besluit = vervangen_op(plek)?;

        // Wat het besluit meteen inroosterde. Bij een verplichting met
        // `vanaf: bekendmaking` is deze lijst leeg en staat alles nog te wachten.
        uit_de_beschikkingen.push((plek, gram));
        neem_termijnen(
            gram,
            besluit,
            plek,
            op_moment,
            na_het_besluit,
            &mut termijnen,
        );

        match zaak.bekendmaking {
            // De bekendmaking heeft de wachtende verplichtingen hun vervaldata
            // gegeven; die staan in háár gram, en het besluit-gram zegt er nog
            // steeds dat ze wachtten. Een gram verandert niet, dus wat er waar
            // ligt is hier de enige manier om te weten of het nog wacht.
            Some((bekend, bekendmaking)) if !vervangen_voor_bekendmaking(bekendmaking) => {
                uit_de_beschikkingen.push((bekend, bekendmaking));
                let na_de_bekendmaking = vervangen_op(bekend)?;
                neem_termijnen(
                    bekendmaking,
                    besluit,
                    bekend,
                    op_moment,
                    na_de_bekendmaking,
                    &mut termijnen,
                );
            }
            // Nog niet bekendgemaakt, of bekendgemaakt terwijl het al vervangen
            // was: wat erop wachtte, staat als zodanig in de lijst. Vervallen als
            // de bekendmaking dat zegt, of — zonder bekendmaking — als er al een
            // besluit in de plaats van dit besluit kwam: een besluit werkt pas
            // door zijn bekendmaking (Awb 3:40), dus wat erop wachtte gaat dan
            // nooit meer lopen, en een latere bekendmaking verandert daar niets
            // aan. Eén regel per wachtende verplichting en niet per termijn —
            // hoevéél termijnen het er worden staat vast, wannéér ze vervallen
            // niet, en vier regels zonder vervaldag zeggen vier keer hetzelfde.
            bekendmaking => {
                if let Some(gelezen) = bekendmaking {
                    uit_de_beschikkingen.push(gelezen);
                }
                let vervallen = bekendmaking.is_some() || na_het_besluit.is_some();
                for wachtend in wachtende_verplichtingen(&gram.fields).unwrap_or_default() {
                    termijnen.push(Termijn {
                        besluit: besluit.clone(),
                        soort: wachtend.soort,
                        volgnummer: Value::Int(wachtend.eerste_volgnummer),
                        vervaldatum: None,
                        bedrag: wachtend.bedrag,
                        betaald: Decimal::ZERO,
                        gedekt_door: Vec::new(),
                        uit_gram: plek,
                        vervallen,
                    });
                }
            }
        }
    }

    // De betalingen per termijn, in de volgorde van hun stroom. Ook bij een
    // termijn die zo meteen vervallen blijkt: of ze betaald is, beslist mee over
    // haar stand (zie [`Termijn::status`]).
    for termijn in &mut termijnen {
        if termijn.vervaldatum.is_none() {
            continue;
        }
        for (plek, betaling) in &betalingen {
            if !hoort_bij(betaling, termijn, &cell.id) {
                continue;
            }
            let Some(bedrag) =
                chronicle::field(&betaling.fields, BEDRAG).and_then(Value::as_decimal)
            else {
                continue;
            };
            termijn.reken_toe(*plek, bedrag);
        }
    }

    // Wat de betalingen bijdroegen, alleen bij termijnen die meetellen: zo telt
    // de uitleg op tot hetzelfde `betaald` als het antwoord — beide richtingen
    // samen, want elke betaling hoort bij één termijn en die bij één richting.
    let mut gelezen_betalingen: BTreeMap<usize, Decimal> = BTreeMap::new();
    for termijn in termijnen.iter().filter(|termijn| termijn.verwacht()) {
        for (plek, gedekt) in &termijn.gedekt_door {
            *gelezen_betalingen.entry(*plek).or_default() += *gedekt;
        }
    }

    // Elk gelezen decretogram met wat er op dit moment uit verwacht werd: de
    // termijnen die het oplegt en die vervielen zonder te vervallen. Nul is een
    // volwaardige bijdrage — een besluit waarvan de eerste termijn nog moet
    // komen, is wél gelezen.
    let mut gelezen: Vec<GebruiktGram> = Vec::new();
    for (plek, gram) in uit_de_beschikkingen {
        let bijdrage: Decimal = termijnen
            .iter()
            .filter(|termijn| termijn.uit_gram == plek)
            .map(Termijn::verwacht_bedrag)
            .sum();
        gelezen.push(
            GebruiktGram::new(&cell.id, BESCHIKKINGEN, plek, gram).met_bijdrage(amount(bijdrage)),
        );
    }

    // De betalingen achter de decretogrammen, elk in de volgorde van hun eigen
    // stroom: een lezer loopt eerst na wat er opgelegd is en dan wat erop ligt.
    for (plek, betaling) in &betalingen {
        let Some(bedrag) = gelezen_betalingen.get(plek) else {
            continue;
        };
        gelezen.push(
            GebruiktGram::new(&cell.id, BETALINGEN, *plek, betaling).met_bijdrage(amount(*bedrag)),
        );
    }

    // Op vervaldatum, en daarbinnen op besluit en volgnummer: de volgorde waarin
    // een lezer ze verwacht, en niet die waarin de grammen toevallig liggen.
    termijnen.sort_by_key(|termijn| {
        let (wacht, vervaldatum, besluit, volgnummer) = termijn.sleutel();
        (wacht, vervaldatum, besluit.to_string(), volgnummer)
    });

    let mut waarden = BTreeMap::from([(
        TERMIJNEN.to_string(),
        Value::Array(
            termijnen
                .iter()
                .map(|termijn| termijn.as_value(op_moment))
                .collect(),
        ),
    )]);
    // Per richting, en nooit over de twee heen: een terugvordering loopt de
    // andere kant op, en haar bij de betalingen optellen zou een bedrag noemen
    // dat niemand verschuldigd is.
    for richting in &RICHTINGEN {
        let van_deze_kant = || {
            termijnen
                .iter()
                .filter(|termijn| termijn.soort == richting.soort)
        };
        let verwacht: Decimal = van_deze_kant().map(Termijn::verwacht_bedrag).sum();
        // Nooit meer dan verwacht: wat er bovenop een termijn ligt, is geen
        // vermindering van wat er nog openstaat. Zonder deze grens zou een
        // dubbele betaling op de ene termijn de andere stil laten verdwijnen.
        let betaald: Decimal = van_deze_kant()
            .filter(|termijn| termijn.verwacht())
            .map(|termijn| termijn.betaald.min(termijn.bedrag))
            .sum();
        waarden.insert(richting.verwacht.to_string(), amount(verwacht));
        waarden.insert(richting.betaald.to_string(), amount(betaald));
        waarden.insert(richting.openstaand.to_string(), amount(verwacht - betaald));
    }

    Ok((
        LexostatusOutcome::Established(waarden),
        Reductie::vastgesteld(vorm, gelezen),
    ))
}

/// Eén besluit over deze zaak, met het laatste gram van elk van zijn stages.
struct Zaak<'a> {
    /// De besluit-definitie waar deze grammen bij horen.
    naam: String,
    /// Het gram van het besluit zelf, met zijn plek in de stroom.
    besluit: Option<(usize, &'a ChronicleEvent)>,
    /// Het gram van de bekendmaking, als die op dit moment al gedaan was.
    bekendmaking: Option<(usize, &'a ChronicleEvent)>,
}

/// Per besluitnaam het laatste gram van elke stage, met zijn plek in de stroom.
///
/// Het láátste en niet allemaal: twee grammen van hetzelfde besluit in dezelfde
/// stage over dezelfde zaak zijn een herziening, en die vervangt wat er stond.
/// Alle termijnen van beide meetellen zou het bedrag verdubbelen.
///
/// **Per stage** en niet per besluit, want de twee grammen van één besluit zeggen
/// verschillende dingen: het besluit wat het oplegde, de bekendmaking wat daarvan
/// is gaan lopen. Alleen de laatste van de twee nemen zou, afhankelijk van de
/// volgorde, de ene of de andere helft van het verhaal weggooien.
///
/// Bij gelijke momenten wint de latere plek in de kroniek. Een kroniek groeit
/// achteraan, dus dat is de latere vastlegging — dezelfde regel als elders.
fn laatste_per_besluit(grammen: Vec<(usize, &ChronicleEvent)>) -> Vec<Zaak<'_>> {
    let mut per_besluit: BTreeMap<String, Zaak<'_>> = BTreeMap::new();
    for (plek, gram) in grammen {
        let besluit = gram_tekst(gram, BESLUIT).to_string();
        let zaak = per_besluit.entry(besluit.clone()).or_insert_with(|| Zaak {
            naam: besluit,
            besluit: None,
            bekendmaking: None,
        });
        // Een gram zonder stage bestaat niet meer sinds de bekendmaking een eigen
        // gram is; staat er toch een andere waarde, dan hoort ze niet bij een van
        // de twee stages die deze reductie leest en doet ze niet mee.
        let plek_voor_stage = match gram_tekst(gram, STAGE) {
            STAGE_BESLUIT => &mut zaak.besluit,
            STAGE_BEKENDMAKING => &mut zaak.bekendmaking,
            _ => continue,
        };
        match plek_voor_stage {
            Some((eerder, bekend)) if (bekend.op_moment, *eerder) >= (gram.op_moment, plek) => {}
            slot => *slot = Some((plek, gram)),
        }
    }
    let mut gekozen: Vec<Zaak<'_>> = per_besluit.into_values().collect();
    gekozen.sort_by_key(|zaak| zaak.besluit.map(|(plek, _)| plek));
    gekozen
}

/// Neem de termijnen uit dit gram over die op dit moment vervallen waren.
///
/// `vervangen_op` is het moment waarop een later besluit in de plaats kwam van
/// wat dit gram beloofde, als dat er is. Een termijn waarvan de vervaldag toen
/// nog moest komen, is vervallen — precies wat de wereld deed toen dat besluit
/// genomen werd (zie `World::decide`): ze stond nog in de wachtrij en ging eruit.
/// Een termijn die toen al verstreken was, stond daar niet meer: wat betaald is,
/// is betaald, en wat niet nagekomen is, is niet nagekomen.
fn neem_termijnen(
    gram: &ChronicleEvent,
    besluit: &str,
    plek: usize,
    op_moment: NaiveDate,
    vervangen_op: Option<NaiveDate>,
    termijnen: &mut Vec<Termijn>,
) {
    for due in obligations(gram) {
        if let Some(mut termijn) = lees_termijn(besluit, due, plek, op_moment) {
            termijn.vervallen = vervangen_op
                .zip(termijn.vervaldatum)
                .is_some_and(|(moment, vervaldatum)| moment < vervaldatum);
            termijnen.push(termijn);
        }
    }
}

/// Zegt deze bekendmaking dat er niets meer ging lopen?
///
/// Een bekendmaking van een besluit dat al vervangen was, roostert niets in en
/// schrijft op waardoor ([`TERMIJNEN_VERVALLEN_DOOR`]). Wat er op haar wachtte,
/// is dan vervallen en niet ingeroosterd — en dat staat in háár gram, dus hier
/// hoeft niets opnieuw afgeleid te worden.
fn vervangen_voor_bekendmaking(bekendmaking: &ChronicleEvent) -> bool {
    chronicle::field(&bekendmaking.fields, TERMIJNEN_VERVALLEN_DOOR)
        .is_some_and(|door| !matches!(door, Value::Null))
}

/// De termijnen die een decretogram oplegt, ongelezen.
///
/// Leeg waar het veld ontbreekt of geen lijst is: een gram zonder verplichtingen
/// is het gewone geval (een afwijzing legt er geen op), en dat hoort geen fout te
/// zijn.
fn obligations(gram: &ChronicleEvent) -> &[Value] {
    match chronicle::field(&gram.fields, OBLIGATIONS) {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

/// Eén termijn uit een decretogram, als ze op dit moment vervallen is.
///
/// `None` voor een termijn die pas later vervalt — die is nog niets verwacht — en
/// voor een termijn waaruit geen datum, geen bedrag of geen soort te lezen is.
/// Dat laatste is hier geen fout: het decretogram schrijft deze velden zelf (zie
/// `ObligationDue::as_value`), dus een gram zonder is er een uit een startstand,
/// en een reductie is niet de plek om dat te beoordelen. Een ontbrekende soort
/// wordt daarbij niet als betaling gelezen: zonder soort is niet te zeggen welke
/// kant de termijn op loopt, en raden zou haar bij de verkeerde richting tellen.
fn lees_termijn(besluit: &str, due: &Value, plek: usize, op_moment: NaiveDate) -> Option<Termijn> {
    let Value::Object(velden) = due else {
        return None;
    };
    let vervaldatum = chronicle::field(velden, VERVALDATUM)
        .and_then(Value::as_str)
        .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())?;
    if vervaldatum > op_moment {
        return None;
    }
    let bedrag = chronicle::field(velden, BEDRAG).and_then(Value::as_decimal)?;
    let soort = chronicle::field(velden, SOORT)
        .and_then(Value::as_str)
        .and_then(ObligationKind::from_name)?;
    Some(Termijn {
        besluit: besluit.to_string(),
        soort,
        volgnummer: chronicle::field(velden, VOLGNUMMER)
            .cloned()
            .unwrap_or(Value::Null),
        vervaldatum: Some(vervaldatum),
        bedrag,
        betaald: Decimal::ZERO,
        gedekt_door: Vec::new(),
        uit_gram: plek,
        vervallen: false,
    })
}

/// Hoort deze betaling bij deze termijn?
///
/// Besluit, besluitende cel én volgnummer, precies de sleutel waarmee de klok
/// een termijn als nagekomen herkent (zie `Cell::already_settled`): dezelfde
/// zaak met hetzelfde volgnummer van hetzelfde besluit van deze cel is dezelfde
/// termijn. De cel hoort erbij omdat haar betalingenstroom ook betalingen kan
/// dragen die zij voor het besluit van een ander deed. De zaak staat al vast —
/// beide stromen zijn op de sleutel gefilterd.
fn hoort_bij(betaling: &ChronicleEvent, termijn: &Termijn, cel: &str) -> bool {
    let tekst_van = |name| chronicle::field(&betaling.fields, name).and_then(Value::as_str);
    let volgnummer = chronicle::field(&betaling.fields, VOLGNUMMER);
    tekst_van(BESLUIT) == Some(termijn.besluit.as_str())
        && tekst_van(BESLUIT_CEL) == Some(cel)
        && volgnummer.is_some_and(|value| crate::values::equivalent(value, &termijn.volgnummer))
}

/// Waarom deze vorm niets vaststelde, zo precies dat het na te lopen is.
///
/// Over de beschikkingenstroom en niet over de betalingen: zonder besluit is er
/// geen verplichting, en dan zegt een betalingenstroom niets. Wie dit leest, moet
/// weten dat hij naar het besluit moet zoeken en niet naar een betaling.
fn niets_opgelegd(key: &str, key_value: &Value, op_moment: NaiveDate) -> String {
    let mut reason = format!(
        "kroniekstroom '{BESCHIKKINGEN}' heeft op of vóór {op_moment} geen decretogram \
         met {key} '{key_value}'"
    );
    let _ = write!(
        reason,
        "; zonder besluit is er over deze zaak niets opgelegd en staat er dus niets open"
    );
    reason
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Een vervallen termijn van 300 met deze betalingen erop.
    fn vervallen_termijn(betalingen: &[Decimal]) -> Termijn {
        let bedrag = Decimal::from(300);
        let mut termijn = Termijn {
            besluit: "voorschot".to_string(),
            soort: ObligationKind::Betaling,
            volgnummer: Value::Int(4),
            vervaldatum: NaiveDate::from_ymd_opt(2025, 1, 1),
            bedrag,
            betaald: Decimal::ZERO,
            gedekt_door: Vec::new(),
            uit_gram: 0,
            vervallen: true,
        };
        for (plek, betaling) in betalingen.iter().enumerate() {
            termijn.reken_toe(plek, *betaling);
        }
        termijn
    }

    /// Wat er op een vervallen termijn ligt, telt mee — ook een deel ervan —
    /// en de rest staat niet open.
    #[test]
    fn een_deelbetaling_op_een_vervallen_termijn_telt_mee_zonder_rest() {
        let op_moment = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap_or_default();

        let deels = vervallen_termijn(&[Decimal::from(100)]);
        assert!(deels.verwacht(), "de betaling hoort in de uitleg te staan");
        assert_eq!(deels.verwacht_bedrag(), Decimal::from(100));
        assert_eq!(deels.status(op_moment), Status::Vervallen);

        let geheel = vervallen_termijn(&[Decimal::from(300)]);
        assert_eq!(geheel.verwacht_bedrag(), Decimal::from(300));
        assert_eq!(geheel.status(op_moment), Status::Betaald);

        let niets = vervallen_termijn(&[]);
        assert!(!niets.verwacht());
        assert_eq!(niets.verwacht_bedrag(), Decimal::ZERO);
        assert_eq!(niets.status(op_moment), Status::Vervallen);
    }
}
