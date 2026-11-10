//! Country presets: what each European authority tells households to plan
//! for, copied from `docs/guidance/presets.json` (tests/country.rs keeps the
//! two in step). Only verified guides carry numbers (AGENTS.md rule 7); the
//! others give their link and nothing else. Using a country's numbers only
//! sets the plan length and the drinking-water amount, and adds the cash
//! targets a guide gives an amount for: every target stays the household's to
//! edit.

use crate::plan::queries::Target;

/// How far an authority's guide could be read, as in `docs/guidance/README.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Verified,
    Partial,
    Unverified,
}

/// One authority's guidance.
///
/// - `code`: the country code used in `presets.json`; stored in the `country` setting.
/// - `name`: English; translated on the page.
/// - `authority`: as the authority names itself, in its own language.
/// - `days`: the length it recommends planning for; verified guides only.
/// - `water`: drinking water in litres per person per day; verified guides only.
/// - `cash`: (target mode, amount, currency) where a verified guide gives an amount.
#[derive(Debug, Clone, Copy)]
pub struct Country {
    pub code: &'static str,
    pub name: &'static str,
    pub authority: &'static str,
    pub url: &'static str,
    pub status: Status,
    pub days: Option<i64>,
    pub water: Option<f64>,
    pub cash: &'static [(&'static str, f64, &'static str)],
}

/// Every authority in `presets.json`: the EU first, then the countries by
/// their English name.
pub const COUNTRIES: &[Country] = &[
    Country {
        code: "EU",
        name: "European Union",
        authority: "European Commission",
        url: "https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:52025JC0130",
        status: Status::Verified,
        days: Some(3),
        water: None,
        cash: &[],
    },
    Country {
        code: "AT",
        name: "Austria",
        authority: "Österreichischer Zivilschutzverband",
        url: "https://www.zivilschutz.at",
        status: Status::Partial,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "BE",
        name: "Belgium",
        authority: "Nationaal Crisiscentrum / Centre de crise national",
        url: "https://crisiscentrum.be/nl/wat-kan-jij-doen/maak-een-noodpakket",
        status: Status::Verified,
        days: Some(3),
        water: Some(3.0),
        cash: &[],
    },
    Country {
        code: "BG",
        name: "Bulgaria",
        authority: "ГДПБЗН",
        url: "https://www.mvr.bg/gdpbzn/info-center/pravila-povedenie",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "HR",
        name: "Croatia",
        authority: "Ravnateljstvo civilne zaštite",
        url: "https://civilna-zastita.gov.hr/upute-za-gradjane-5467/5467",
        status: Status::Verified,
        days: Some(2),
        water: Some(1.5),
        cash: &[],
    },
    Country {
        code: "CY",
        name: "Cyprus",
        authority: "Πολιτική Άμυνα",
        url: "https://www.moi.gov.cy/moi/cd/cd.nsf",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "CZ",
        name: "Czechia",
        authority: "Ministerstvo vnitra, HZS ČR",
        url: "https://www.72h.gov.cz",
        status: Status::Verified,
        days: Some(3),
        water: Some(2.0),
        cash: &[],
    },
    Country {
        code: "DK",
        name: "Denmark",
        authority: "Beredskabsstyrelsen",
        url: "https://www.borger.dk/politi-retsvaesen-forsvar/forsvar-og-beredskab/forberedt-paa-kriser",
        status: Status::Verified,
        days: Some(3),
        water: Some(3.0),
        cash: &[],
    },
    Country {
        code: "EE",
        name: "Estonia",
        authority: "Riigikantselei, Päästeamet",
        url: "https://kriis.ee/en/security-situation-europe/preparing-possible-crises/how-can-i-prepare-possible-crises",
        status: Status::Verified,
        days: Some(7),
        water: Some(3.0),
        cash: &[],
    },
    Country {
        code: "FI",
        name: "Finland",
        authority: "Sisäministeriö; SPEK 72 tuntia",
        url: "https://www.suomi.fi/guides/preparedness",
        status: Status::Verified,
        days: Some(3),
        water: None,
        cash: &[],
    },
    Country {
        code: "FR",
        name: "France",
        authority: "Secrétariat général de la défense et de la sécurité nationale",
        url: "https://www.info.gouv.fr/risques/se-preparer-a-une-situation-durgence",
        status: Status::Verified,
        days: Some(3),
        water: Some(2.0),
        cash: &[],
    },
    Country {
        code: "DE",
        name: "Germany",
        authority: "Bundesamt für Bevölkerungsschutz und Katastrophenhilfe",
        url: "https://www.bbk.bund.de/DE/Warnung-Vorsorge/Vorsorge/Checkliste/checkliste_node.html",
        status: Status::Verified,
        days: Some(10),
        water: Some(2.0),
        cash: &[],
    },
    Country {
        code: "GR",
        name: "Greece",
        authority: "Γενική Γραμματεία Πολιτικής Προστασίας",
        url: "https://civilprotection.gov.gr/odigies-prostasias/genikes-odigies",
        status: Status::Partial,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "HU",
        name: "Hungary",
        authority: "BM Országos Katasztrófavédelmi Főigazgatóság",
        url: "https://www.katasztrofavedelem.hu/62/lakossagfelkeszites",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "IS",
        name: "Iceland",
        authority: "Almannavarnadeild ríkislögreglustjóra",
        url: "https://www.almannavarnir.is/frettir/almannavarnadeild-gefur-ut-baeklinginn-verum-vidbuin/",
        status: Status::Partial,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "IE",
        name: "Ireland",
        authority: "Roinn Cosanta, Oifig um Pleanáil Éigeandála",
        url: "https://www.gov.ie/en/office-of-emergency-planning/campaigns/be-emergency-ready/",
        status: Status::Verified,
        days: None,
        water: Some(2.0),
        cash: &[],
    },
    Country {
        code: "IT",
        name: "Italy",
        authority: "Dipartimento della Protezione Civile",
        url: "https://iononrischio.protezionecivile.gov.it/en/video/emergency-kit/",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "LV",
        name: "Latvia",
        authority: "Aizsardzības ministrija",
        url: "https://www.mod.gov.lv/sites/mod/files/document/72H%20buklets_ENG_WEB_2024.pdf",
        status: Status::Verified,
        days: Some(3),
        water: Some(3.0),
        cash: &[],
    },
    Country {
        code: "LI",
        name: "Liechtenstein",
        authority: "Amt für Bevölkerungsschutz",
        url: "https://www.llv.li/de/landesverwaltung/amt-fuer-bevoelkerungsschutz",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "LT",
        name: "Lithuania",
        authority: "Priešgaisrinės apsaugos ir gelbėjimo departamentas",
        url: "https://lt72.lt",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "LU",
        name: "Luxembourg",
        authority: "Haut-Commissariat à la protection nationale",
        url: "https://letzprepare.public.lu/fr.html",
        status: Status::Partial,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "MT",
        name: "Malta",
        authority: "Dipartiment tal-Protezzjoni Ċivili",
        url: "https://civilprotection.gov.mt",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "NL",
        name: "Netherlands",
        authority: "Ministerie van Justitie en Veiligheid",
        url: "https://www.denkvooruit.nl/bereid-je-voor/stel-je-noodpakket-samen",
        status: Status::Verified,
        days: Some(3),
        water: Some(3.0),
        cash: &[("per_adult", 70.0, "EUR"), ("per_child", 30.0, "EUR")],
    },
    Country {
        code: "NO",
        name: "Norway",
        authority: "Direktoratet for samfunnssikkerhet og beredskap",
        url: "https://www.dsb.no/sikkerhverdag/egenberedskap/",
        status: Status::Verified,
        days: Some(7),
        water: None,
        cash: &[],
    },
    Country {
        code: "PL",
        name: "Poland",
        authority: "Rządowe Centrum Bezpieczeństwa",
        url: "https://www.gov.pl/attachment/c151a713-13d2-490b-81f3-74b565d248f5",
        status: Status::Verified,
        days: Some(3),
        water: Some(3.0),
        cash: &[],
    },
    Country {
        code: "PT",
        name: "Portugal",
        authority: "Autoridade Nacional de Emergência e Proteção Civil",
        url: "https://prociv.gov.pt",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "RO",
        name: "Romania",
        authority: "Departamentul pentru Situații de Urgență",
        url: "https://fiipregatit.ro",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "SK",
        name: "Slovakia",
        authority: "Ministerstvo vnútra SR",
        url: "https://www.minv.sk/?civilna-ochrana",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "SI",
        name: "Slovenia",
        authority: "Uprava RS za zaščito in reševanje",
        url: "https://www.gov.si/teme/bodite-pripravljeni-ob-nesrecah/",
        status: Status::Verified,
        days: Some(3),
        water: Some(7.5),
        cash: &[],
    },
    Country {
        code: "ES",
        name: "Spain",
        authority: "Protección Civil y Emergencias",
        url: "https://www.proteccioncivil.es/gestion-riesgos/recomendaciones",
        status: Status::Unverified,
        days: None,
        water: None,
        cash: &[],
    },
    Country {
        code: "SE",
        name: "Sweden",
        authority: "Myndigheten för civilt försvar",
        url: "https://www.krisinformation.se/din-beredskap/hemberedskap/",
        status: Status::Verified,
        days: Some(7),
        water: Some(3.0),
        cash: &[("per_adult", 1000.0, "SEK")],
    },
    Country {
        code: "CH",
        name: "Switzerland",
        authority: "Bundesamt für wirtschaftliche Landesversorgung",
        url: "https://www.bwl.admin.ch/bwl/de/home/themen/notvorrat.html",
        status: Status::Verified,
        days: Some(7),
        water: Some(3.0),
        cash: &[],
    },
];

pub fn find(code: &str) -> Option<&'static Country> {
    COUNTRIES.iter().find(|c| c.code == code)
}

/// What using a country's numbers would change in the plan.
///
/// - `days`: (now, then), when the plan length differs.
/// - `water`: the drinking-water target if there is one, its amount now, and
///   the country's amount, when they differ.
/// - `cash`: cash targets to add, for each mode the household has no cash
///   target for yet.
#[derive(Debug, Default, PartialEq)]
pub struct Changes {
    pub days: Option<(i64, i64)>,
    pub water: Option<(Option<i64>, Option<f64>, f64)>,
    pub cash: Vec<(&'static str, f64, &'static str)>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.days.is_none() && self.water.is_none() && self.cash.is_empty()
    }
}

/// The drinking-water target: the first target in the water category counted
/// per person per day, in litres.
pub fn water_target(targets: &[Target]) -> Option<&Target> {
    targets
        .iter()
        .find(|t| t.category == "water" && t.mode == "per_person_day" && t.unit == "L")
}

/// The changes the country's numbers make to a plan of `days` with these
/// targets. Without numbers (a guide not verified) nothing changes.
pub fn changes(c: &Country, days: i64, targets: &[Target]) -> Changes {
    let water_now = water_target(targets);
    Changes {
        days: c.days.filter(|d| *d != days).map(|d| (days, d)),
        water: c
            .water
            .filter(|w| water_now.is_none_or(|t| (t.amount - w).abs() > 1e-9))
            .map(|w| (water_now.map(|t| t.id), water_now.map(|t| t.amount), w)),
        cash: c
            .cash
            .iter()
            .filter(|(mode, ..)| {
                !targets
                    .iter()
                    .any(|t| t.category == "cash" && t.mode == *mode)
            })
            .copied()
            .collect(),
    }
}
