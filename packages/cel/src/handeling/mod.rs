//! Handelingen in een zaak: op proef, zonder vastleggen, en genomen, waarna
//! het proces de cel laat vastleggen.
//!
//! `behandeling.handelingen` in `proces.yaml` noemt per handeling een artikel
//! (een regeling en uitkomsten) en het event waarin de cel haar vastlegt. Wat
//! een handeling nodig heeft en van wie, staat daar niet: het volgt uit de
//! stage van het event (RFC-008) en uit de origin van de parameters (RFC-043).
//! Er zijn drie soorten ([`Handelingsoort`]), en het event zegt welke:
//!
//! - **Het besluit**: het event heeft een stage die de procedure van het
//!   artikel kent, en het is de eerste zo'n handeling op dat artikel. Het
//!   formulier zijn de oordelen (origin `OORDEEL`); wat een latere stage pas
//!   vraagt, is nog niet gebeurd ([`nog_niet`]).
//! - **Een vervolg**: een latere stage van hetzelfde besluit, zoals de
//!   bekendmaking. De engine voert die stage uit op de invoer van het
//!   vastgelegde besluit (RFC-008, `execute_stage`), met wat de stage vraagt
//!   (`requires`) als formulier; de haken die de wet op die stage laat vuren
//!   (zoals de bezwaartermijn, Awb 6:8) leveren hun uitkomsten mee.
//! - **Een feit** uit het verloop van de zaak: het event heeft geen stage.
//!   Het formulier zijn de velden van het event die geen uitkomst zijn; op
//!   proef laat de cel de lexostatussen van de zaak reduceren alsof het feit
//!   al vastlag, zodat de uitkomsten laten zien wat het feit doet.
//!
//! Een parameter komt uit precies een bron: een lexostatus van de zaak, een
//! andere synthese-bron, de synthese per regel, het formulier of de stand van
//! wat nog niet gebeurd is. Er wordt niets aangevuld. Staat het artikel van
//! de handeling in de grondslag van het event en is het een TOETS, dan telt
//! elke booleaanse uitkomst: onwaar is een conclusie van het proces
//! ([`toetsen`]). Zo zegt de wet dat een betaling boven het vastgestelde
//! bedrag niet overeenkomstig de vaststelling is (Awb 4:52), niet de
//! configuratie.
//!
//! Het proces concludeert voor het handelt, en weigert niets wat gebeurd is.
//! Zegt de proef inhoudelijk nee (een toets is onwaar, een haak geeft geen
//! waarde), dan doet het
//! proces de handeling niet uit zichzelf (`te_nemen` is onwaar). Meldt de
//! behandelaar dat het feit toch gebeurde (`gebeurd: true`), dan legt de cel
//! het vast, en tonen de lexostatussen de gevolgen: een betaling boven het
//! bedrag is onverschuldigd betaald, een bekendmaking die niet aan de wet
//! voldoet laat geen bezwaartermijn lopen. Alleen wat de vorm raakt, houdt
//! het vastleggen tegen: een formulier dat niet is ingevuld, een uitkomst
//! die de wet niet volledig kan uitrekenen (een waarde of een bron mist),
//! een vervolg zonder besluit, een moment in de toekomst of voor de zaak. Een besluit
//! neemt het proces zelf; dat wordt niet gemeld.
//!
//! Het proces leest de zaak niet: wat het over de zaak weet (welke stages er
//! liggen, wat het besluit vastlegde, hoeveel grammen), leidt de cel af in
//! haar lexostatus [`Zaakstand`].

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use regelrecht_engine::{
    ExecutionOutcome, HookPoint, LawExecutionService, StageState, Value as EngineValue,
};
use regelrecht_law_model::{ParameterType, ProcedureDefinition};

use crate::cel::Cel;
use crate::celclient::{self, Besluitvelden, MetYaml, Vastlegverzoek};
use crate::config::{HandelingDefinitie, Handelingsoort, NogNiet, ProcesDefinitie};
use crate::datum::{self, Tijdpunt};
use crate::formulier::{leesbaar, Veld};
use crate::gezag::{self, Bevoegdheid};
use crate::gram::{GeladenRegeling, Gram, HandelendeActor, Invoer, Receipt, StroomVerwijzing};
use crate::kanaal::Sessie;
use crate::proces::Proces;
use crate::reductie::{Besluitstand, Lexostatus, Peil, Zaakstand};
use crate::regelingen::{self, Benodigd, Waardetype};
use crate::rijen::{self, Rijen};
use crate::stroom::{Besluit, Binding, Event, Zaak};
use crate::synthese::{self, Bron, BronUitslag, Herkomst};
use crate::toets;
use crate::transport::{Transport, TransportFout};

mod controle;
mod laden;
mod neem;
mod proef;
mod stand;

// Hulpfuncties die meer dan een deel gebruikt.
use laden::{artikel_met, uitkomsten_van};
use proef::{eigen, event_velden};
use stand::al_genomen;

pub use controle::{bronnen_voor, controleer};
pub use laden::{
    benodigd, bereid_voor, haken_op, nog_niet, procedure_van, toetsen, veldsoort, zet_formulier,
};
pub use neem::{neem, Genomen};
pub use proef::{doel, proef};
pub use stand::{
    besluiten_in_zaak, procedure_van_de_zaak, stand, BesluitInZaak, ProcedureStand,
    Rechtsbescherming, StageStand, Stand,
};

/// Het antwoord op een handeling op proef.
#[derive(Debug, Clone, Serialize)]
pub struct Proefhandeling {
    pub handeling: String,
    #[serde(flatten)]
    pub soort: Handelingsoort,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub regeling: String,
    /// `<regeling>#<artikel>` van de uitkomsten.
    pub artikel: String,
    /// De dag waarop de engine de regeling leest en de cellen peilen.
    pub peildatum: String,
    /// Waar de peildatum vandaan komt: het `op_moment` van het event, met
    /// zijn grondslag, of vandaag.
    pub peildatum_uit: String,
    /// Of het proces de handeling uit zichzelf neemt: elke uitkomst heeft een
    /// waarde, elke toets is waar en elk feit is ingevuld.
    pub te_nemen: bool,
    /// Niet te nemen om de inhoud, niet om de vorm: meldt de behandelaar dat
    /// het feit toch gebeurde (`gebeurd: true`), dan legt de cel het vast.
    /// Nooit bij een besluit.
    pub te_melden: bool,
    /// De uitkomsten met een waarde, ook als de handeling niet te nemen is.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub uitkomsten: BTreeMap<String, Value>,
    /// De toetsen van het artikel (zie [`toetsen`]) met hun waarde.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub toetsen: BTreeMap<String, Value>,
    /// Het type en de eenheid van elke uitkomst en toets, uit de regeling:
    /// een bedrag in eurocent toont de frontend in euro.
    pub typen: BTreeMap<String, Waardetype>,
    /// Wat de engine miste.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    /// Wat naar de engine ging, en per parameter waar het vandaan kwam.
    pub parameters: BTreeMap<String, Value>,
    pub herkomst: BTreeMap<String, Herkomst>,
    pub bronnen: Vec<BronUitslag>,
    /// Parameters die de aanroeper van het artikel moet leveren, zonder
    /// waarde uit een bron.
    pub niet_geleverd: Vec<Benodigd>,
    /// De lexostatussen van de zaak, bij een feit met het concept erbij.
    pub lexostatussen: Vec<Lexostatus>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rijen: Vec<rijen::Uitslag>,
    /// Het vastgelegde besluit waarop de handeling handelt: bij een vervolg
    /// het besluit waarop de stage verdergaat, bij een feit het besluit dat
    /// het volgt (zoals de betaling die het uitvoert), bij een wijziging het
    /// besluit dat zij wijzigt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub besluit: Option<BesluitVerwijzing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Een vastgelegd besluit in de zaak, zoals de cel het in de [`Zaakstand`]
/// noemt.
#[derive(Debug, Clone, Serialize)]
pub struct BesluitVerwijzing {
    pub besluitkenmerk: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub op_moment: String,
    pub vastgelegd_op: String,
}

/// Een fout in de vraag of een stand die de handeling niet toelaat.
#[derive(Debug, Clone, PartialEq)]
pub enum Weigering {
    /// Het formulier noemt iets dat de handeling niet vraagt, of een waarde
    /// die de cel niet in een gram kan zetten.
    Ongeldig(String),
    /// Niet te nemen: iets mist, een toets is onwaar, of een feit is niet
    /// ingevuld. Er wordt geen gram vastgelegd.
    NietTeNemen(String),
    /// De cel weigert: de stage ligt al vast in de zaak, of de zaak veranderde
    /// sinds het proces haar las.
    Conflict(String),
    /// De wet wijst een ander bevoegd gezag aan dan de actor van het proces.
    Onbevoegd(String),
    /// De configuratie, of de cel: haar kroniek, een reductie of het
    /// vastleggen.
    Cel(String),
}

/// Wat de behandelaar bij een handeling opgeeft: het formulier, zo nodig
/// het besluit waarop zij handelt (een besluitkenmerk; zonder het laatste,
/// zie [`doel`]), en bij het nemen of het feit toch gebeurde (zie [`neem`]).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Opgave {
    #[serde(default)]
    pub formulier: Map<String, Value>,
    #[serde(default)]
    pub besluitkenmerk: Option<String>,
    #[serde(default)]
    pub gebeurd: bool,
}

/// Wat een handeling nodig heeft van de runtime: de cel, de bronnen en de
/// synthese per regel van deze handeling, de geladen regelingen (voor het
/// receipt) en het moment van nu.
pub struct Omgeving<'a> {
    pub proces: &'a Proces,
    pub cel: &'a dyn Transport,
    pub bronnen: &'a [Bron],
    pub rijen: &'a [Rijen],
    pub regelingen: &'a [GeladenRegeling],
    pub nu: DateTime<FixedOffset>,
}
