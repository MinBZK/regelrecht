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
    pub(super) fn name(self) -> String {
        match self {
            Uitvoering::Toets => "assessment".into(),
            Uitvoering::Aanbod => "offer".into(),
            Uitvoering::Handeling(h) => h.name.clone(),
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
    cell: String,
    lexostatus: String,
    /// De url van een bron buiten deze runtime; `None`: intern.
    url: Option<String>,
}

/// Een leverancier van een parameter bij een uitvoering.
#[derive(Debug, Clone)]
pub(super) enum Levering {
    /// Een afleiding van een eigen lexostatus, of een tabel per regel uit een
    /// eigen extra veld.
    Own {
        lexostatus: String,
        gelezen: Gelezen,
    },
    /// De stand bij besluit.
    Stand,
    /// De keuze van het tijdvak in het portaal.
    Choice,
    /// Het id van het besluit waarop de handeling handelt.
    Decision,
    Bron(BronLevering),
}

impl Levering {
    pub(super) fn woorden(&self) -> String {
        match self {
            Levering::Own {
                lexostatus,
                gelezen,
            } => format!("eigen lexostatus {lexostatus} ({})", gelezen.woorden()),
            Levering::Stand => "de stand bij besluit".into(),
            Levering::Choice => "de keuze in het portaal".into(),
            Levering::Decision => "het besluit waarop de handeling handelt".into(),
            Levering::Bron(b) => format!("synthese-bron {}/{}", b.cell, b.lexostatus),
        }
    }

    /// Of deze leverancier bij de herkomst past (een register-bron nog
    /// zonder de controle op het register).
    pub(super) fn past(&self, g: &Geldend, uitvoering: Uitvoering<'_>) -> bool {
        match (self, g.origin.waarde) {
            (
                Levering::Own {
                    gelezen: Gelezen::Kanaal,
                    ..
                },
                OriginValue::Kanaal | OriginValue::Belanghebbende,
            )
            | (
                Levering::Own {
                    gelezen: Gelezen::Indiening,
                    ..
                },
                OriginValue::Belanghebbende,
            )
            | (
                Levering::Own {
                    gelezen: Gelezen::Verloop,
                    ..
                },
                OriginValue::Dossier,
            )
            | (Levering::Bron(_), OriginValue::Register) => true,
            (Levering::Stand | Levering::Decision, OriginValue::Dossier) => {
                uitvoering.is_handeling()
            }
            (Levering::Choice, OriginValue::Belanghebbende) => {
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
pub(super) static KEUZE: Levering = Levering::Choice;

impl Leveranciers {
    pub(super) fn voeg_toe(&mut self, name: &str, l: Levering) {
        self.per_parameter
            .entry(name.to_string())
            .or_default()
            .push(l);
    }

    pub(super) fn van(d: &ProcesDefinitie, cell: &Cell, uitvoering: Uitvoering<'_>) -> Self {
        let mut l = Leveranciers::default();
        let mut eigen: Vec<&str> = d.zaakbronnen().map(|b| b.lexostatus.as_str()).collect();
        if let Some(p) = &d.portal {
            l.keuze =
                uitvoering.is_aanbod() && p.offer.as_ref().is_some_and(|a| a.windows.is_some());
            eigen.push(&p.assessment.lexostatus);
        }
        eigen.sort_unstable();
        eigen.dedup();
        for name in eigen {
            let Some(def) = cell.lexostatuses.lexostatus(name) else {
                continue;
            };
            for (param, derivation) in &def.reduction.derivations {
                l.voeg_toe(
                    param,
                    Levering::Own {
                        lexostatus: name.to_string(),
                        gelezen: gelezen(cell, &d.actor, def, derivation),
                    },
                );
            }
        }
        for b in d.andere_bronnen() {
            for p in &b.parameters {
                l.voeg_toe(
                    p,
                    Levering::Bron(BronLevering {
                        cell: b.cell.clone(),
                        lexostatus: b.lexostatus.clone(),
                        url: b.url.clone(),
                    }),
                );
            }
        }
        if let Uitvoering::Handeling(h) = uitvoering {
            for name in h.not_yet.keys() {
                l.voeg_toe(name, Levering::Stand);
            }
            if let Some(p) = &h.decision_parameter {
                l.voeg_toe(p, Levering::Decision);
            }
        }
        // De synthese per regel levert alleen aan de uitvoering die haar
        // uitvoert: de toets of het besluit.
        let rows: &[RijenDefinitie] = match uitvoering {
            Uitvoering::Toets => d.portal.as_ref().map(|p| p.assessment.rows.as_slice()),
            Uitvoering::Handeling(h) => Some(h.rows.as_slice()),
            Uitvoering::Aanbod => None,
        }
        .unwrap_or_default();
        for r in rows {
            l.rows(d, cell, r);
        }
        l
    }

    /// De synthese per regel: uit een extra veld van een bron een levering
    /// van die bron, uit een eigen tabel een eigen levering, naar wat het
    /// extra veld leest.
    pub(super) fn rows(&mut self, d: &ProcesDefinitie, cell: &Cell, r: &RijenDefinitie) {
        let source = d.andere_bronnen().find(|b| {
            b.lexostatus == r.table.lexostatus && b.extra_fields.contains(&r.table.field)
        });
        let levering = match source {
            Some(b) => Levering::Bron(BronLevering {
                cell: b.cell.clone(),
                lexostatus: b.lexostatus.clone(),
                url: b.url.clone(),
            }),
            None => {
                let gelezen = cell
                    .lexostatuses
                    .lexostatus(&r.table.lexostatus)
                    .and_then(|def| {
                        def.alle_afleidingen()
                            .find(|(name, _)| **name == r.table.field)
                            .map(|(_, a)| gelezen(cell, &d.actor, def, a))
                    })
                    .unwrap_or(Gelezen::Onbepaald);
                Levering::Own {
                    lexostatus: r.table.lexostatus.clone(),
                    gelezen,
                }
            }
        };
        self.voeg_toe(&r.parameter, levering);
    }

    /// Wie een parameter levert. De keuze in het portaal levert alleen het
    /// tijdvak.
    pub(super) fn van_parameter(&self, name: &str, window: bool) -> Vec<&Levering> {
        let mut uit: Vec<&Levering> = self.per_parameter.get(name).into_iter().flatten().collect();
        if self.keuze && window {
            uit.push(&KEUZE);
        }
        uit
    }
}

/// Of een gram van dit event door een filter kan komen, voor zover dat
/// zonder de invoer vaststaat: een `$`-waarde past altijd, een veldpad als
/// het event het veld heeft.
pub(super) fn kan_passen(filter: &Filter, stream: &Stroom, event: &Event) -> bool {
    filter.iter().all(|(sleutel, value)| {
        let input = value.starts_with('$');
        match event.kenmerk(stream, sleutel) {
            None => event.heeft_pad(sleutel),
            Some(Eventkenmerk::Vast(w)) => {
                if input {
                    w.is_some()
                } else {
                    w == Some(value.as_str())
                }
            }
            Some(Eventkenmerk::Vrij) => true,
            Some(Eventkenmerk::Nooit) => false,
        }
    })
}

/// Wat een afleiding leest: de events van de kroniek van de lexostatus die
/// door het filter van de lexostatus en dat van de afleiding kunnen komen.
/// Leest ze alleen `$intake` van een indiening, dan is het de login.
pub(super) fn gelezen(
    cell: &Cell,
    actor: &str,
    def: &LexostatusDefinitie,
    a: &Afleiding,
) -> Gelezen {
    let events: Vec<(&Stroom, &Event)> = cell
        .streams
        .iter()
        .filter(|s| s.chronicle == def.reduction.chronicle)
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
                    .map(|b| b.path)
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
    Past { warnings: Vec<String> },
    /// Een leverancier past niet bij de herkomst, ook als een andere wel
    /// past.
    Verkeerd(String),
    /// Geen leverancier; de tekst begint met `: ` of is leeg.
    Standalone(String),
}

/// Of een parameter een leverancier heeft die bij zijn herkomst past, en
/// geen die er niet bij past.
pub(super) fn leverancier(
    uitvoering: Uitvoering<'_>,
    name: &str,
    g: &Geldend,
    l: &Leveranciers,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> Uitslag {
    let leveringen = l.van_parameter(name, g.is_tijdvak());
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
                warnings: Vec::new(),
            }
        } else {
            Uitslag::Standalone(
                ": een oordeel geeft de behandelaar pas bij een handeling in de zaak".into(),
            )
        };
    }
    let mut verkeerd = Vec::new();
    let mut past = false;
    let mut warnings = Vec::new();
    for lv in leveringen {
        if !lv.past(g, uitvoering) {
            verkeerd.push(format!("hij komt uit {}", lv.woorden()));
            continue;
        }
        match lv {
            Levering::Bron(source) => match register_bron(source, g, cells) {
                Ok(w) => {
                    past = true;
                    warnings.extend(w);
                }
                Err(r) => verkeerd.push(r),
            },
            _ => past = true,
        }
    }
    if !verkeerd.is_empty() {
        Uitslag::Verkeerd(verkeerd.join("; "))
    } else if past {
        Uitslag::Past { warnings }
    } else {
        Uitslag::Standalone(String::new())
    }
}

/// Of een synthese-bron een register-parameter mag leveren: haar lexostatus
/// houdt een kroniek bij met een grondslag in het register van de herkomst.
/// Onder welke naam de afnemer het feit vraagt, zegt de synthese van het
/// proces (de vertaling hoort bij de afnemer). `Ok` met
/// een waarschuwing als dat niet na te gaan is: een bron met een url, of een
/// interne cel die niet in deze runtime draait.
pub(super) fn register_bron(
    source: &BronLevering,
    g: &Geldend,
    cells: &BTreeMap<String, Arc<Cell>>,
) -> Result<Option<String>, String> {
    let wie = format!("synthese-bron {}/{}", source.cell, source.lexostatus);
    let register = g.origin.register.as_deref().unwrap_or_default();
    if let Some(url) = &source.url {
        return Ok(Some(format!(
            "{wie} draait buiten deze runtime ({url}); of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is bij het opstarten niet te zien"
        )));
    }
    let Some(cell) = cells.get(&source.cell) else {
        return Ok(Some(format!(
            "{wie} heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is niet te zien"
        )));
    };
    let Some(def) = cell.lexostatuses.lexostatus(&source.lexostatus) else {
        return Err(format!("{wie} bestaat niet"));
    };
    // `vorm` houdt een REGISTER zonder register al tegen.
    if let Some(register) = &g.origin.register {
        let houdt_bij = cell
            .streams
            .iter()
            .filter(|s| s.chronicle == def.reduction.chronicle)
            .flat_map(|s| s.events.iter())
            .flat_map(|e| e.legal_basis.iter())
            .any(|gr| regelingen::ontleed(gr).is_ok_and(|gr| gr.regulation == register));
        if !houdt_bij {
            return Err(format!(
                "{wie} houdt geen kroniek bij met een grondslag in '{register}'"
            ));
        }
    }
    Ok(None)
}
