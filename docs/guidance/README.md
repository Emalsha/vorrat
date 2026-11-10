# Emergency preparedness guidance in Europe

What the EU and each EU and EEA country officially tell households to keep at
home for a crisis, collected so Vorrat can offer presets that follow the
authorities instead of inventing numbers (rule 7 in [AGENTS.md](../../AGENTS.md)).

- [COUNTRIES.md](COUNTRIES.md): one section per country, with the source, the
  recommended days, water, and every item the authority lists.
- [presets.json](presets.json): the same data for the app. The country page
  copies each authority's link, status, days and water from it
  (`src/country/rules.rs`; `tests/country.rs` checks they match). Each item has a category (the target categories Vorrat uses,
  plus medicine, documents, cash, pets and others), a `basis` saying how it
  scales (per person per day, per adult, per child, per pet, per household),
  a quantity only where the authority gives one, `applies_to` profile
  triggers, and its own source link. `basis_note` in the file explains the
  rules.

Compiled on 2026-11-08. Guidance changes (Sweden, the Netherlands, Belgium,
Ireland, Luxembourg and Iceland all published new material in 2025 and 2026),
so treat each entry as true on its date and check the source before relying on
it. This is planning help, not official advice: the app should always link to
the authority.

## How to read the status

| Status | Meaning | Use in the app |
|---|---|---|
| verified | Days, water and items read from the official page, or an official copy of the same leaflet, on the compiled date | Can feed presets |
| partial | Source read, but only part of the list was available | Only what is listed |
| unverified | Authority and link known; the page could not be read or has no household list | Link only, no numbers |

Most government sites refused automated reading from this environment, so some
entries were read from official copies (for example the BBK checklist hosted
by a THW branch). Each entry names exactly what was read. Unverified entries
need a person with a browser, or a later pass from a network that allows those
sites.

## At a glance

| Country | Authority | Days | Water (L per person per day) | Status |
|---|---|---|---|---|
| [European Union](COUNTRIES.md#eu) | [European Commission and High Representative](https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:52025JC0130) | 3 | – | verified |
| [Austria](COUNTRIES.md#at) | [Austrian Civil Protection Association](https://www.zivilschutz.at) | 7 | – | partial |
| [Belgium](COUNTRIES.md#be) | [National Crisis Centre](https://crisiscentrum.be/nl/wat-kan-jij-doen/maak-een-noodpakket) | 3 | 3 | verified |
| [Bulgaria](COUNTRIES.md#bg) | [General Directorate Fire Safety and Civil Protection](https://www.mvr.bg/gdpbzn/info-center/pravila-povedenie) | – | – | unverified |
| [Croatia](COUNTRIES.md#hr) | [Civil Protection Directorate](https://civilna-zastita.gov.hr/upute-za-gradjane-5467/5467) | 2 | 1.5 | verified |
| [Cyprus](COUNTRIES.md#cy) | [Civil Defence](https://www.moi.gov.cy/moi/cd/cd.nsf) | – | – | unverified |
| [Czechia](COUNTRIES.md#cz) | [Ministry of the Interior with the Fire Rescue Service](https://www.72h.gov.cz) | 3 | 2 | verified |
| [Denmark](COUNTRIES.md#dk) | [Danish Emergency Management Agency](https://www.borger.dk/politi-retsvaesen-forsvar/forsvar-og-beredskab/forberedt-paa-kriser) | 3 | 3 | verified |
| [Estonia](COUNTRIES.md#ee) | [Government Office and Rescue Board](https://kriis.ee/en/security-situation-europe/preparing-possible-crises/how-can-i-prepare-possible-crises) | 7 | 3 | verified |
| [Finland](COUNTRIES.md#fi) | [Ministry of the Interior (with SPEK's 72 hours campaign)](https://www.suomi.fi/guides/preparedness) | 3 | – | verified |
| [France](COUNTRIES.md#fr) | [Government (SGDSN guide 'Tous responsables')](https://www.info.gouv.fr/risques/se-preparer-a-une-situation-durgence) | 3 | 2 | verified |
| [Germany](COUNTRIES.md#de) | [Federal Office of Civil Protection and Disaster Assistance (BBK)](https://www.bbk.bund.de/DE/Warnung-Vorsorge/Vorsorge/Checkliste/checkliste_node.html) | 10 | 2 | verified |
| [Greece](COUNTRIES.md#gr) | [General Secretariat for Civil Protection](https://civilprotection.gov.gr/odigies-prostasias/genikes-odigies) | – | – | partial |
| [Hungary](COUNTRIES.md#hu) | [National Directorate General for Disaster Management](https://www.katasztrofavedelem.hu/62/lakossagfelkeszites) | – | – | unverified |
| [Iceland](COUNTRIES.md#is) | [Department of Civil Protection and Emergency Management](https://www.almannavarnir.is/frettir/almannavarnadeild-gefur-ut-baeklinginn-verum-vidbuin/) | – | – | partial |
| [Ireland](COUNTRIES.md#ie) | [Department of Defence, Office of Emergency Planning](https://www.gov.ie/en/office-of-emergency-planning/campaigns/be-emergency-ready/) | – | 2 | verified |
| [Italy](COUNTRIES.md#it) | [Civil Protection Department](https://iononrischio.protezionecivile.gov.it/en/video/emergency-kit/) | – | – | unverified |
| [Latvia](COUNTRIES.md#lv) | [Ministry of Defence](https://www.mod.gov.lv/sites/mod/files/document/72H%20buklets_ENG_WEB_2024.pdf) | 3 | 3 | verified |
| [Liechtenstein](COUNTRIES.md#li) | [Office of Civil Protection](https://www.llv.li/de/landesverwaltung/amt-fuer-bevoelkerungsschutz) | – | – | unverified |
| [Lithuania](COUNTRIES.md#lt) | [Fire and Rescue Department under the Ministry of the Interior](https://lt72.lt) | – | – | unverified |
| [Luxembourg](COUNTRIES.md#lu) | [High Commission for National Protection (HCPN)](https://letzprepare.public.lu/fr.html) | 3 | – | partial |
| [Malta](COUNTRIES.md#mt) | [Civil Protection Department](https://civilprotection.gov.mt) | – | – | unverified |
| [Netherlands](COUNTRIES.md#nl) | [Ministry of Justice and Security (campaign 'Denk vooruit')](https://www.denkvooruit.nl/bereid-je-voor/stel-je-noodpakket-samen) | 3 | 3 | verified |
| [Norway](COUNTRIES.md#no) | [Directorate for Civil Protection (DSB)](https://www.dsb.no/sikkerhverdag/egenberedskap/) | 7 | – | verified |
| [Poland](COUNTRIES.md#pl) | [Government Centre for Security (RCB), 'Poradnik bezpieczeństwa'](https://www.gov.pl/attachment/c151a713-13d2-490b-81f3-74b565d248f5) | 3 | 3 | verified |
| [Portugal](COUNTRIES.md#pt) | [National Emergency and Civil Protection Authority (ANEPC)](https://prociv.gov.pt) | – | – | unverified |
| [Romania](COUNTRIES.md#ro) | [Department for Emergency Situations (DSU), 'Fii pregătit'](https://fiipregatit.ro) | – | – | unverified |
| [Slovakia](COUNTRIES.md#sk) | [Ministry of the Interior, civil protection](https://www.minv.sk/?civilna-ochrana) | – | – | unverified |
| [Slovenia](COUNTRIES.md#si) | [Administration for Civil Protection and Disaster Relief (URSZR)](https://www.gov.si/teme/bodite-pripravljeni-ob-nesrecah/) | 3 | 7.5 | verified |
| [Spain](COUNTRIES.md#es) | [Directorate-General for Civil Protection and Emergencies](https://www.proteccioncivil.es/gestion-riesgos/recomendaciones) | – | – | unverified |
| [Sweden](COUNTRIES.md#se) | [Swedish Civil Defence Agency (MCF, formerly MSB)](https://www.krisinformation.se/din-beredskap/hemberedskap/) | 7 | 3 | verified |
| [Switzerland (not EU/EEA)](COUNTRIES.md#ch) | [Federal Office for National Economic Supply (BWL)](https://www.bwl.admin.ch/bwl/de/home/themen/notvorrat.html) | 7 | 3 | verified |

"Days" is the length the authority recommends planning for. Water is
the drinking figure where the source gives one; some figures are derived from a
total (France 6 L for 72 hours, Estonia 21 L for a week) and say so in
COUNTRIES.md. Switzerland is not in the EU or EEA but is included because
Vorrat already offers its one-week preset.

## What the guidance agrees on

**72 hours is the floor, a week is common.** The EU's Preparedness Union
Strategy (March 2025) sets "a population self-sufficiency of minimum 72 hours"
as the goal and promises guidelines for Member States, not yet published.
Belgium, Czechia, Denmark, Finland, France, Latvia, Luxembourg, the
Netherlands, Poland and Slovenia say three days. Sweden, Norway, Estonia,
Switzerland and Austria say a week. Germany says ten days, with three as a
first step. Croatia's survival kit is for two days. Latvia asks for three days
"or preferably for a week".

**Water is about 3 litres per person per day for drinking.** Belgium, Denmark,
Latvia, the Netherlands, Poland, Sweden and Switzerland say 3 L; Germany,
Czechia, France and Ireland say 2 L (Ireland 2 to 3 L). Several add water for
washing and the toilet on top: Belgium up to 10 L a day in total, Czechia 10 to
15 L, Latvia 10 to 12 L, Slovenia 7.5 L as a short-term minimum and 20 L after
that. Latvia adds a litre in hot weather.

**Only a few give food weights.** Germany's BBK lists kilograms per person for
ten days, which is exactly where Vorrat's default targets come from (see
`DEFAULT_TARGETS` in `src/db/schema.rs`: the BBK figures divided by ten). Slovenia
has a 30-day table for one adult and Austria's 2012 brochure a 14-day one. All
others name kinds of food: long-life, needs little or no cooking, and what the
household normally eats.

**Medicine is personal and lasts longer than the plan.** Every country asks
for personal medicines and a first-aid kit. The length varies: three days
(Belgium, Denmark), a week (Czechia), ten days (Germany), a month for
long-term prescriptions (Sweden). Iodine tablets appear only with conditions:
when the authorities say so (Belgium), or for people under 40, pregnant or
breastfeeding (Denmark, Finland).

**The same non-food items everywhere.** Battery or wind-up radio, torch and
batteries, power bank, candles and matches, a way to cook without power,
blankets and warm clothes, hygiene items and toilet paper, copies of documents
in a waterproof bag, phone numbers on paper, and some cash in small notes.
Three give amounts for cash: the Netherlands (70 euro per adult and 30 per
child for 72 hours), Sweden (1,000 kronor per adult) and Latvia (a week's cash
in 5, 10 and 20 euro notes).

**People and animals change the list.** Babies (formula, baby food, nappies),
children (Czech toys and games, Polish ID tags, Dutch child cash), older or ill
people (Danish extra supplies, Polish hearing-aid and glucose-sensor
batteries, spare glasses in Czechia, Croatia and France), pregnant or
breastfeeding people (iodine), and pets: almost every authority mentions pet
food and water, and Belgium, Czechia, Poland and Sweden add a carrier, lead,
records and medicine. Sweden notes that pets are not allowed in public
shelters.

**A grab bag is separate from the home stock.** Belgium, Czechia, Ireland,
Latvia, the Netherlands and Poland describe a bag for leaving quickly, often
one per person, with documents, medicines, water, cash and a radio.

**Plans and upkeep matter as much as stock.** Most ask for a household plan
(meeting point, who needs help, how to reach each other without a phone
network) and talk to neighbours. The Netherlands says to check the kit every
six months, France once or twice a year, Ireland to rotate stock.

## What this means for Vorrat

Vorrat already follows the guidance in three places: the default targets are
the BBK checklist, the plan presets (72 hours, one week, ten days) match the
EU, Swedish and Swiss, and German advice, and the printed checklist's extras
cover the common non-food items. What the guidance points to next, most useful
first:

1. **Household profile.** Plans are "N people" today. The guidance scales by
   adults, children, babies and pets, and switches items on for babies,
   pregnancy, older people, care needs, glasses or hearing aids, and each kind
   of pet. `presets.json` already tags items with a `basis` (per adult, per
   child, per pet...) and `applies_to` triggers for this.
2. ~~**Country presets.**~~ Done: the country page sets the plan length and
   drinking water from a verified guide and links to the authority. Only
   verified entries give numbers, and the household can still edit everything.
3. **Medicine and personal needs.** Per person, generic names only (never a
   drug or dose beyond what the authority says), with its own days of supply
   separate from the plan length, expiry and a refill reminder. This is the
   most private data in the app, so it stays local and is never shown in the
   digest.
4. ~~**Water for drinking and for hygiene.**~~ Done: a "washing water"
   category and target next to drinking water, offered on the plan page with
   the guides' figures, and a hot-weather note.
5. ~~**Cash target.**~~ Done: targets per adult and per child, cash counted
   in its own currency, offered on the plan page and set from the Dutch and
   Swedish amounts on the country page.
6. ~~**Grab bag per person.**~~ Done: each person's bag is a place of its
   own, with a printable checklist ticked by the stock in it.
7. ~~**Documents checklist.**~~ Done: a documents list on the essentials
   page, each copy an item whose place says where it is; no scans.
8. ~~**Household plan page.**~~ Done: meeting points, phone numbers, a
   contact outside the area, shut-offs, who needs help and neighbours, with a
   printable version.
9. ~~**Review reminders.**~~ Done: a twice-yearly kit check on the plan
   page (rotate water, test torches, radio and batteries), reminded once
   through the ntfy digest when it is due.
10. **Needs no cooking.** A flag on items so the power-cut view and the plan
    can show what can be eaten without power (Danish and Belgian advice).
11. **Country information.** Emergency numbers, the national alert system
    (NL-Alert, BE-Alert), radio advice and iodine guidance, as static text
    that works offline.
12. **More languages.** Authorities publish in many languages (the Dutch
    booklet in nine); the countries with the most detailed lists suggest
    Swedish, Polish, Czech, Danish and Latvian next.

Items 1 and 3 needed somewhere to store new data (schema v3 and v4); 6, 7
and 8 fit in the existing tables. Under rule 1 a new
table means a new schema version, which is Emalsha's decision; the plan in
[PLAN.md](../PLAN.md) owns that design.

## Not done yet

- **No usable list read:** Bulgaria, Cyprus, Hungary, Italy, Liechtenstein,
  Lithuania, Malta, Portugal, Romania, Slovakia and Spain. Their sites refused
  automated reading or have no household list on the pages found. News reports
  a 72-hour recommendation in Cyprus, Portugal and Romania; that is not used
  here until the official page is read.
- **Partial:** Austria (current food quantities), Greece (only basic items on
  the official page), Iceland (new booklet at verumvidbuin.is not read),
  Luxembourg (the guide's equipment list), Norway (medicine and equipment
  pages).
- **To check before use:** Austria's 2012 food weights and Slovenia's 30-day
  table are noted in COUNTRIES.md but not copied into items.
- **EU guidelines:** the Commission's promised 72-hour guidelines should
  replace the EU entry when published.
