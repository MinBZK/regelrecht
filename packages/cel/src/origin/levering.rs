//! Wie een parameter kan leveren: een afleiding van een eigen lexostatus, een
//! synthese-bron, het formulier, de stand bij besluit of de keuze in het
//! portaal, en of die leverancier bij de herkomst past.

use super::*;

/// Een uitkomst die het proces uitvoert.
#[derive(Debug, Clone, Copy)]
pub enum Uitvoering<'a> {
    Toets,
    Aanbod,
    /// Een handeling in een zaak (geen vervolg: dat rekent op de invoer van
    /// het vastgelegde besluit).
    Handeling(&'a HandelingDefinitie),
}

impl Uitvoering<'_> {
    pub(super) fn naam(self) -> String {
        match self {
            Uitvoering::Toets => "toets".into(),
            Uitvoering::Aanbod => "aanbod".into(),
            Uitvoering::Handeling(h) => h.naam.clone(),
        }
    }

    pub(super) fn is_aanbod(self) -> bool {
        matches!(self, Uitvoering::Aanbod)
    }

    pub(super) fn is_handeling(self) -> bool {
        matches!(self, Uitvoering::Handeling(_))
    }
}

/// Wat een afleiding van een eigen lexostatus leest, naar de grammen die door
/// haar filters kunnen komen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gelezen {
    /// Alleen `$intake` van een indiening: de login.
    Kanaal,
    /// Alleen indieningen: wat de aanvrager aanlevert.
    Indiening,
    /// Alleen andere grammen van de eigen actor: het verloop van de zaak.
    Verloop,
    /// Beide, of niets dat statisch na te gaan is.
    Onbepaald,
}

impl Gelezen {
    pub(super) fn woorden(self) -> &'static str {
        match self {
            Gelezen::Kanaal => "de login ($intake)",
            Gelezen::Indiening => "wat de aanvrager indiende",
            Gelezen::Verloop => "het verloop van de zaak",
            Gelezen::Onbepaald => "indieningen en het verloop van de zaak door elkaar",
        }
    }
}

/// Een synthese-bron die een parameter levert.
#[derive(Debug, Clone)]
pub(super) struct BronLevering {
    cel: String,
    lexostatus: String,
    /// De url van een bron buiten deze runtime; `None`: intern.
    url: Option<String>,
}

/// Een leverancier van een parameter bij een uitvoering.
#[derive(Debug, Clone)]
pub(super) enum Levering {
    /// Een afleiding van een eigen lexostatus, of een tabel per regel uit een
    /// eigen extra veld.
    Eigen {
        lexostatus: String,
        gelezen: Gelezen,
    },
    /// De stand bij besluit.
    Stand,
    /// De keuze van het tijdvak in het portaal.
    Keuze,
    Bron(BronLevering),
}

impl Levering {
    pub(super) fn woorden(&self) -> String {
        match self {
            Levering::Eigen {
                lexostatus,
                gelezen,
            } => format!("eigen lexostatus {lexostatus} ({})", gelezen.woorden()),
            Levering::Stand => "de stand bij besluit".into(),
            Levering::Keuze => "de keuze in het portaal".into(),
            Levering::Bron(b) => format!("synthese-bron {}/{}", b.cel, b.lexostatus),
        }
    }

    /// Of deze leverancier bij de herkomst past (een register-bron nog
    /// zonder de controle op het register).
    pub(super) fn past(&self, g: &Geldend, uitvoering: Uitvoering<'_>) -> bool {
        match (self, g.origin.waarde) {
            (
                Levering::Eigen {
                    gelezen: Gelezen::Kanaal,
                    ..
                },
                OriginValue::Kanaal | OriginValue::Belanghebbende,
            )
            | (
                Levering::Eigen {
                    gelezen: Gelezen::Indiening,
                    ..
                },
                OriginValue::Belanghebbende,
            )
            | (
                Levering::Eigen {
                    gelezen: Gelezen::Verloop,
                    ..
                },
                OriginValue::Dossier,
            )
            | (Levering::Bron(_), OriginValue::Register) => true,
            (Levering::Stand, OriginValue::Dossier) => uitvoering.is_handeling(),
            (Levering::Keuze, OriginValue::Belanghebbende) => {
                uitvoering.is_aanbod() && g.is_tijdvak()
            }
            _ => false,
        }
    }
}

/// Wat het proces bij een uitvoering kan leveren, per parameter.
#[derive(Debug, Default)]
pub(super) struct Leveranciers {
    per_parameter: BTreeMap<String, Vec<Levering>>,
    /// Of het beleid bij het aanbod tijdvakken aanbiedt (`aanbod.tijdvakken`).
    keuze: bool,
}

/// De keuze van het tijdvak, als leverancier.
pub(super) static KEUZE: Levering = Levering::Keuze;

impl Leveranciers {
    pub(super) fn voeg_toe(&mut self, naam: &str, l: Levering) {
        self.per_parameter
            .entry(naam.to_string())
            .or_default()
            .push(l);
    }

    pub(super) fn van(d: &ProcesDefinitie, cel: &Cel, uitvoering: Uitvoering<'_>) -> Self {
        let mut l = Leveranciers::default();
        let mut eigen: Vec<&str> = d.zaakbronnen().map(|b| b.lexostatus.as_str()).collect();
        if let Some(p) = &d.portaal {
            l.keuze =
                uitvoering.is_aanbod() && p.aanbod.as_ref().is_some_and(|a| a.tijdvakken.is_some());
            eigen.push(&p.toets.lexostatus);
        }
        eigen.sort_unstable();
        eigen.dedup();
        for naam in eigen {
            let Some(def) = cel.lexostatussen.lexostatus(naam) else {
                continue;
            };
            for (param, afleiding) in &def.reduction.afleidingen {
                l.voeg_toe(
                    param,
                    Levering::Eigen {
                        lexostatus: naam.to_string(),
                        gelezen: gelezen(cel, &d.actor, def, afleiding),
                    },
                );
            }
        }
        for b in d.andere_bronnen() {
            for p in &b.parameters {
                l.voeg_toe(
                    p,
                    Levering::Bron(BronLevering {
                        cel: b.cel.clone(),
                        lexostatus: b.lexostatus.clone(),
                        url: b.url.clone(),
                    }),
                );
            }
        }
        if let Uitvoering::Handeling(h) = uitvoering {
            for naam in h.nog_niet.keys() {
                l.voeg_toe(naam, Levering::Stand);
            }
        }
        // De synthese per regel levert alleen aan de uitvoering die haar
        // uitvoert: de toets of het besluit.
        let rijen: &[RijenDefinitie] = match uitvoering {
            Uitvoering::Toets => d.portaal.as_ref().map(|p| p.toets.rijen.as_slice()),
            Uitvoering::Handeling(h) => Some(h.rijen.as_slice()),
            Uitvoering::Aanbod => None,
        }
        .unwrap_or_default();
        for r in rijen {
            l.rijen(d, cel, r);
        }
        l
    }

    /// De synthese per regel: uit een extra veld van een bron een levering
    /// van die bron, uit een eigen tabel een eigen levering, naar wat het
    /// extra veld leest.
    pub(super) fn rijen(&mut self, d: &ProcesDefinitie, cel: &Cel, r: &RijenDefinitie) {
        let bron = d
            .andere_bronnen()
            .find(|b| b.lexostatus == r.tabel.lexostatus && b.extra_velden.contains(&r.tabel.veld));
        let levering = match bron {
            Some(b) => Levering::Bron(BronLevering {
                cel: b.cel.clone(),
                lexostatus: b.lexostatus.clone(),
                url: b.url.clone(),
            }),
            None => {
                let gelezen = cel
                    .lexostatussen
                    .lexostatus(&r.tabel.lexostatus)
                    .and_then(|def| {
                        def.alle_afleidingen()
                            .find(|(naam, _)| **naam == r.tabel.veld)
                            .map(|(_, a)| gelezen(cel, &d.actor, def, a))
                    })
                    .unwrap_or(Gelezen::Onbepaald);
                Levering::Eigen {
                    lexostatus: r.tabel.lexostatus.clone(),
                    gelezen,
                }
            }
        };
        self.voeg_toe(&r.parameter, levering);
    }

    /// Wie een parameter levert. De keuze in het portaal levert alleen het
    /// tijdvak.
    pub(super) fn van_parameter(&self, naam: &str, tijdvak: bool) -> Vec<&Levering> {
        let mut uit: Vec<&Levering> = self.per_parameter.get(naam).into_iter().flatten().collect();
        if self.keuze && tijdvak {
            uit.push(&KEUZE);
        }
        uit
    }
}

/// Of een gram van dit event door een filter kan komen, voor zover dat
/// zonder de invoer vaststaat: een `$`-waarde past altijd, een veldpad als
/// het event het veld heeft.
pub(super) fn kan_passen(filter: &Filter, stroom: &Stroom, event: &Event) -> bool {
    filter.iter().all(|(sleutel, waarde)| {
        if waarde.starts_with('$') {
            return match sleutel.as_str() {
                "zaakkenmerk" => event.zaak.heeft_kenmerk(),
                "name" | "type" | "soort" | "stage" | "recording_actor" | "chronicle" => true,
                pad => event.heeft_pad(pad),
            };
        }
        match sleutel.as_str() {
            "name" => event.name == *waarde,
            "type" => event.type_ == *waarde,
            "soort" => event.soort.as_deref() == Some(waarde.as_str()),
            "stage" => event.stage.as_deref() == Some(waarde.as_str()),
            "zaakkenmerk" => event.zaak.heeft_kenmerk(),
            "recording_actor" => stroom.recording_actor == *waarde,
            "chronicle" => stroom.chronicle == *waarde,
            pad => event.heeft_pad(pad),
        }
    })
}

/// Wat een afleiding leest: de events van de kroniek van de lexostatus die
/// door het filter van de lexostatus en dat van de afleiding kunnen komen.
/// Leest ze alleen `$intake` van een indiening, dan is het de login.
pub(super) fn gelezen(cel: &Cel, actor: &str, def: &LexostatusDefinitie, a: &Afleiding) -> Gelezen {
    let events: Vec<(&Stroom, &Event)> = cel
        .strommen
        .iter()
        .filter(|s| s.chronicle == def.reduction.kroniek)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| {
            kan_passen(&def.reduction.filter, s, e)
                && a.filter().is_none_or(|f| kan_passen(f, s, e))
        })
        .collect();
    if events.is_empty() {
        return Gelezen::Onbepaald;
    }
    if events.iter().all(|(_, e)| e.type_ == INDIENING) {
        let paden = a.gelezen_paden();
        let alleen_intake = !paden.is_empty()
            && events.iter().all(|(_, e)| {
                let intake: BTreeSet<String> = e
                    .bladeren()
                    .into_iter()
                    .filter(|b| matches!(b.binding, Binding::Intake(_)))
                    .map(|b| b.pad)
                    .collect();
                paden.iter().all(|p| intake.contains(*p))
            });
        return if alleen_intake {
            Gelezen::Kanaal
        } else {
            Gelezen::Indiening
        };
    }
    if events
        .iter()
        .all(|(s, e)| e.type_ != INDIENING && s.recording_actor == actor)
    {
        return Gelezen::Verloop;
    }
    Gelezen::Onbepaald
}

/// Of het aanbod op een parameter mag leunen: wat vooraf vaststaat, is wie
/// inlogt (`KANAAL`), wat een register weet (`REGISTER`) en welk tijdvak de
/// aanvrager kiest (`BELANGHEBBENDE` met `rol: TIJDVAK`). Of een aanvraag
/// volledig is, weet je pas na het invullen.
pub(super) fn vooraf_bekend(g: Option<&Geldend>) -> bool {
    g.is_some_and(|g| {
        matches!(g.origin.waarde, OriginValue::Kanaal | OriginValue::Register) || g.is_tijdvak()
    })
}

/// Hoe het met de leverancier van een parameter zit.
#[derive(Debug)]
pub(super) enum Uitslag {
    /// Een leverancier past, eventueel met wat niet na te gaan is.
    Past { waarschuwingen: Vec<String> },
    /// Een leverancier past niet bij de herkomst, ook als een andere wel
    /// past.
    Verkeerd(String),
    /// Geen leverancier; de tekst begint met `: ` of is leeg.
    Geen(String),
}

/// Of een parameter een leverancier heeft die bij zijn herkomst past, en
/// geen die er niet bij past.
pub(super) fn leverancier(
    uitvoering: Uitvoering<'_>,
    naam: &str,
    g: &Geldend,
    l: &Leveranciers,
    cellen: &BTreeMap<String, Arc<Cel>>,
) -> Uitslag {
    let leveringen = l.van_parameter(naam, g.is_tijdvak());
    if g.origin.waarde == OriginValue::Oordeel {
        if !leveringen.is_empty() {
            let wie: Vec<String> = leveringen.iter().map(|lv| lv.woorden()).collect();
            return Uitslag::Verkeerd(format!(
                "een oordeel geeft de behandelaar in het formulier van de handeling, maar hij komt uit {}",
                wie.join(" en ")
            ));
        }
        return if uitvoering.is_handeling() {
            Uitslag::Past {
                waarschuwingen: Vec::new(),
            }
        } else {
            Uitslag::Geen(
                ": een oordeel geeft de behandelaar pas bij een handeling in de zaak".into(),
            )
        };
    }
    let mut verkeerd = Vec::new();
    let mut past = false;
    let mut waarschuwingen = Vec::new();
    for lv in leveringen {
        if !lv.past(g, uitvoering) {
            verkeerd.push(format!("hij komt uit {}", lv.woorden()));
            continue;
        }
        match lv {
            Levering::Bron(bron) => match register_bron(bron, g, cellen) {
                Ok(w) => {
                    past = true;
                    waarschuwingen.extend(w);
                }
                Err(r) => verkeerd.push(r),
            },
            _ => past = true,
        }
    }
    if !verkeerd.is_empty() {
        Uitslag::Verkeerd(verkeerd.join("; "))
    } else if past {
        Uitslag::Past { waarschuwingen }
    } else {
        Uitslag::Geen(String::new())
    }
}

/// Of een synthese-bron een register-parameter mag leveren: haar lexostatus
/// houdt een kroniek bij met een grondslag in het register van de herkomst.
/// Onder welke naam de afnemer het feit vraagt, zegt de synthese van het
/// proces (de vertaling hoort bij de afnemer). `Ok` met
/// een waarschuwing als dat niet na te gaan is: een bron met een url, of een
/// interne cel die niet in deze runtime draait.
pub(super) fn register_bron(
    bron: &BronLevering,
    g: &Geldend,
    cellen: &BTreeMap<String, Arc<Cel>>,
) -> Result<Option<String>, String> {
    let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
    let register = g.origin.register.as_deref().unwrap_or_default();
    if let Some(url) = &bron.url {
        return Ok(Some(format!(
            "{wie} draait buiten deze runtime ({url}); of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is bij het opstarten niet te zien"
        )));
    }
    let Some(cel) = cellen.get(&bron.cel) else {
        return Ok(Some(format!(
            "{wie} heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is niet te zien"
        )));
    };
    let Some(def) = cel.lexostatussen.lexostatus(&bron.lexostatus) else {
        return Err(format!("{wie} bestaat niet"));
    };
    // `vorm` houdt een REGISTER zonder register al tegen.
    if let Some(register) = &g.origin.register {
        let houdt_bij = cel
            .strommen
            .iter()
            .filter(|s| s.chronicle == def.reduction.kroniek)
            .flat_map(|s| s.events.iter())
            .flat_map(|e| e.grondslag.iter())
            .any(|gr| regelingen::ontleed(gr).is_ok_and(|gr| gr.regeling == register));
        if !houdt_bij {
            return Err(format!(
                "{wie} houdt geen kroniek bij met een grondslag in '{register}'"
            ));
        }
    }
    Ok(None)
}
