//! Barcode → product via Open Food Facts (open, non-profit, no account). Only
//! the barcode is sent. The picture is downloaded and stored locally, so the
//! app never hot-links third-party images.
//!
//! `product_type=all` makes one request cover the sister databases (food,
//! beauty, pet food, other products): Open Food Facts redirects to the right one.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::files;

const OFF_URL: &str = "https://world.openfoodfacts.org/api/v2/product/";
const FIELDS: &str =
    "product_name,product_name_en,product_name_de,generic_name,brands,quantity,categories_tags,\
image_front_small_url,image_front_url,product_type,nutriscore_grade";

/// Open Food Facts category tags → our categories. Specific tags first.
const CATEGORY_MAP: &[(&str, &str)] = &[
    ("waters", "water"),
    ("mineral-waters", "water"),
    ("spring-waters", "water"),
    ("baby-foods", "baby"),
    ("baby-milks", "baby"),
    ("pet-foods", "pet"),
    ("cat-food", "pet"),
    ("dog-food", "pet"),
    ("plant-based-milks", "dairy"),
    ("coffees", "drinks"),
    ("teas", "drinks"),
    ("juices", "drinks"),
    ("sodas", "drinks"),
    ("beers", "drinks"),
    ("wines", "drinks"),
    ("fats", "fats"),
    ("vegetable-oils", "fats"),
    ("olive-oils", "fats"),
    ("butters", "fats"),
    ("dairies", "dairy"),
    ("milks", "dairy"),
    ("cheeses", "dairy"),
    ("yogurts", "dairy"),
    ("fishes", "protein"),
    ("meats", "protein"),
    ("eggs", "protein"),
    ("canned-fishes", "protein"),
    ("legumes", "vegetables"),
    ("vegetables", "vegetables"),
    ("canned-vegetables", "vegetables"),
    ("fruits", "fruit"),
    ("nuts", "fruit"),
    ("dried-fruits", "fruit"),
    ("cereals", "grains"),
    ("pastas", "grains"),
    ("rices", "grains"),
    ("breads", "grains"),
    ("flours", "grains"),
    ("potatoes", "grains"),
    ("breakfast-cereals", "grains"),
    ("meals", "ready meals"),
    ("soups", "ready meals"),
    ("pizzas", "ready meals"),
    ("snacks", "snacks"),
    ("sweets", "snacks"),
    ("chocolates", "snacks"),
    ("biscuits", "snacks"),
    ("beverages", "drinks"),
    ("spices", "spices"),
    ("condiments", "spices"),
    ("sauces", "spices"),
    ("salts", "spices"),
    ("batteries", "batteries"),
    ("light-bulbs", "light"),
    ("candles", "light"),
    ("flashlights", "light"),
    ("first-aid", "first aid"),
    ("bandages", "first aid"),
    ("medicines", "medicine"),
    ("dietary-supplements", "medicine"),
    ("dishwashing", "cleaning"),
    ("detergents", "cleaning"),
    ("laundry", "cleaning"),
    ("cleaning", "cleaning"),
    ("household-products", "cleaning"),
    ("toothpastes", "hygiene"),
    ("shampoos", "hygiene"),
    ("soaps", "hygiene"),
    ("deodorants", "hygiene"),
    ("toilet-paper", "hygiene"),
    ("hygiene", "hygiene"),
    ("cosmetics", "hygiene"),
];

const SUPPLY_CATEGORIES: &[&str] = &[
    "cleaning",
    "hygiene",
    "batteries",
    "light",
    "medicine",
    "first aid",
];

static QTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d+(?:[.,]\d+)?)\s*(kg|g|mg|l|cl|ml)\b").expect("valid"));
static PACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+)\s*[x×*]\s*(\d+(?:[.,]\d+)?\s*(?:kg|g|mg|l|cl|ml)\b)").expect("valid")
});

/// "400 g" → (400, g); "1,5 L" → (1.5, L); "33 cl" → (330, ml).
pub fn parse_quantity(text: &str) -> Option<(f64, &'static str)> {
    let m = QTY.captures(text)?;
    let v: f64 = m[1].replace(',', ".").parse().ok()?;
    Some(match m[2].to_lowercase().as_str() {
        "kg" => (v, "kg"),
        "g" => (v, "g"),
        "mg" => (v / 1000.0, "g"),
        "l" => (v, "L"),
        "cl" => (v * 10.0, "ml"),
        _ => (v, "ml"),
    })
}

/// "6 x 1,5 L" → (Some(6), 1.5, L); plain "400 g" → (None, 400, g).
pub fn parse_pack(text: &str) -> (Option<u32>, Option<(f64, &'static str)>) {
    if let Some(m) = PACK.captures(text) {
        if let Ok(n) = m[1].parse::<u32>() {
            if (2..=48).contains(&n) {
                if let Some(q) = parse_quantity(&m[2]) {
                    return (Some(n), Some(q));
                }
            }
        }
    }
    (None, parse_quantity(text))
}

/// Exact tag matches first ("en:legumes"), then compound tags
/// ("en:laundry-detergents", "en:cereals-and-potatoes").
pub fn map_category(tags: &[String]) -> Option<&'static str> {
    let tags: Vec<&str> = tags
        .iter()
        .map(|t| t.split_once(':').map_or(t.as_str(), |(_, r)| r))
        .collect();
    for (needle, cat) in CATEGORY_MAP {
        if tags.contains(needle) {
            return Some(cat);
        }
    }
    for (needle, cat) in CATEGORY_MAP {
        if tags
            .iter()
            .any(|t| t.starts_with(&format!("{needle}-")) || t.ends_with(&format!("-{needle}")))
        {
            return Some(cat);
        }
    }
    None
}

/// Prefill for a new item.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Found {
    pub name: String,
    pub brand: Option<String>,
    pub barcode: String,
    pub kind: String,
    pub category: Option<String>,
    pub size: Option<f64>,
    pub size_unit: Option<String>,
    /// Pieces in a multipack: the quantity to add.
    pub pack: Option<u32>,
    pub frozen: bool,
    pub nutriscore: Option<String>,
    pub image: Option<String>,
    pub source: String,
}

#[derive(Deserialize)]
struct Resp {
    status: Option<i64>,
    product: Option<Product>,
    product_type: Option<String>,
}

#[derive(Deserialize, Default)]
struct Product {
    product_name: Option<String>,
    product_name_en: Option<String>,
    product_name_de: Option<String>,
    generic_name: Option<String>,
    brands: Option<String>,
    quantity: Option<String>,
    #[serde(default)]
    categories_tags: Vec<String>,
    image_front_small_url: Option<String>,
    image_front_url: Option<String>,
    product_type: Option<String>,
    nutriscore_grade: Option<String>,
}

/// Turn an Open Food Facts response into a prefill (no network).
fn interpret(code: &str, body: &str) -> Option<(Found, Option<String>)> {
    let r: Resp = serde_json::from_str(body).ok()?;
    if r.status != Some(1) {
        return None;
    }
    let p = r.product?;
    let name = [
        &p.product_name,
        &p.product_name_en,
        &p.product_name_de,
        &p.generic_name,
    ]
    .into_iter()
    .flatten()
    .map(|s| s.trim())
    .find(|s| !s.is_empty())?
    .to_string();
    let ptype = p
        .product_type
        .clone()
        .or(r.product_type)
        .unwrap_or_else(|| "food".into());
    let (mut kind, type_cat, source) = match ptype.as_str() {
        "petfood" => ("food", Some("pet"), "Open Pet Food Facts"),
        "beauty" => ("supply", Some("hygiene"), "Open Beauty Facts"),
        "product" => ("thing", None, "Open Products Facts"),
        _ => ("food", None, "Open Food Facts"),
    };
    let category = map_category(&p.categories_tags).or(type_cat);
    if ptype == "product" && category.is_some_and(|c| SUPPLY_CATEGORIES.contains(&c)) {
        kind = "supply";
    }
    let (pack, size) = p
        .quantity
        .as_deref()
        .map(parse_pack)
        .unwrap_or((None, None));
    let grade = p.nutriscore_grade.unwrap_or_default().to_lowercase();
    let found = Found {
        name,
        brand: p
            .brands
            .as_deref()
            .and_then(|b| b.split(',').next())
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty()),
        barcode: code.to_string(),
        kind: kind.into(),
        category: category.map(String::from),
        size: size.map(|s| s.0),
        size_unit: size.map(|s| s.1.to_string()),
        pack,
        frozen: p.categories_tags.iter().any(|t| t.contains("frozen")),
        nutriscore: matches!(grade.as_str(), "a" | "b" | "c" | "d" | "e").then_some(grade),
        image: None,
        source: source.into(),
    };
    Some((found, p.image_front_small_url.or(p.image_front_url)))
}

/// Look a barcode up. `None` when unknown, offline, or not a number.
pub async fn lookup(code: &str, files_dir: &Path, user_agent: &str) -> Option<Found> {
    let code = code.trim();
    if code.is_empty() || code.len() > 32 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(user_agent)
        .build()
        .ok()?;
    let body = client
        .get(format!("{OFF_URL}{code}.json"))
        .query(&[("fields", FIELDS), ("product_type", "all")])
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    let (mut found, img_url) = interpret(code, &body)?;
    if let Some(url) = img_url.filter(|u| u.starts_with("https://")) {
        found.image = fetch_image(&client, &url, files_dir).await;
    }
    Some(found)
}

async fn fetch_image(client: &reqwest::Client, url: &str, dir: &Path) -> Option<String> {
    let r = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let ext = match r
        .headers()
        .get("content-type")?
        .to_str()
        .ok()?
        .split(';')
        .next()?
        .trim()
    {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => return None,
    };
    if r.content_length().is_some_and(|n| n > 5_000_000) {
        return None;
    }
    let bytes = r.bytes().await.ok()?;
    if bytes.len() > 5_000_000 {
        return None;
    }
    let name = files::new_name("img", ext);
    files::write(dir, &name, &bytes).ok()?;
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantities() {
        assert_eq!(parse_quantity("400 g"), Some((400.0, "g")));
        assert_eq!(parse_quantity("1,5 L"), Some((1.5, "L")));
        assert_eq!(parse_quantity("33 cl"), Some((330.0, "ml")));
        assert_eq!(parse_quantity("about a handful"), None);
        assert_eq!(parse_pack("6 x 1,5 L"), (Some(6), Some((1.5, "L"))));
        assert_eq!(parse_pack("4×125 g"), (Some(4), Some((125.0, "g"))));
        assert_eq!(parse_pack("500 g"), (None, Some((500.0, "g"))));
    }

    #[test]
    fn categories() {
        let t = |v: &[&str]| map_category(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            t(&["en:beverages", "en:waters", "en:mineral-waters"]),
            Some("water")
        );
        assert_eq!(t(&["en:laundry-detergents"]), Some("cleaning"));
        assert_eq!(
            t(&["en:plant-based-foods", "en:legumes"]),
            Some("vegetables")
        );
        assert_eq!(t(&["en:unknown"]), None);
    }

    #[test]
    fn interprets_a_water_multipack() {
        let body = r#"{"status":1,"product":{"product_name":"Mineralwasser still","brands":"Gerolsteiner, X",
            "quantity":"6 x 1,5 l","categories_tags":["en:beverages","en:waters"],"nutriscore_grade":"a",
            "image_front_small_url":"https://images.openfoodfacts.org/x.jpg","product_type":"food"}}"#;
        let (f, img) = interpret("4001513000000", body).unwrap();
        assert_eq!(f.name, "Mineralwasser still");
        assert_eq!(f.brand.as_deref(), Some("Gerolsteiner"));
        assert_eq!(
            (f.kind.as_str(), f.category.as_deref()),
            ("food", Some("water"))
        );
        assert_eq!(
            (f.pack, f.size, f.size_unit.as_deref()),
            (Some(6), Some(1.5), Some("L"))
        );
        assert_eq!(f.nutriscore.as_deref(), Some("a"));
        assert!(img.is_some());
    }

    #[test]
    fn interprets_batteries_as_a_supply() {
        let body = r#"{"status":1,"product":{"product_name":"AA Alkaline","categories_tags":["en:batteries"],"product_type":"product"}}"#;
        let (f, _) = interpret("1", body).unwrap();
        assert_eq!(
            (f.kind.as_str(), f.category.as_deref(), f.source.as_str()),
            ("supply", Some("batteries"), "Open Products Facts")
        );
    }

    #[test]
    fn unknown_or_nameless_is_none() {
        assert!(interpret("1", r#"{"status":0}"#).is_none());
        assert!(interpret("1", r#"{"status":1,"product":{"product_name":"  "}}"#).is_none());
        assert!(interpret("1", "not json").is_none());
    }
}
