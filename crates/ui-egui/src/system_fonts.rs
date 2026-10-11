//! Discovery and loading of installed system fonts.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(not(target_arch = "wasm32"))]
fn database() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();

    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn families() -> &'static [String] {
    use std::collections::BTreeSet;

    static NAMES: OnceLock<Vec<String>> = OnceLock::new();

    NAMES.get_or_init(|| {
        let mut names = BTreeSet::new();

        for face in database().faces() {
            for (name, _) in &face.families {
                // Hide internal dot-prefixed font family names.
                if !name.trim_start().starts_with('.') && !name.trim().is_empty() {
                    names.insert(name.clone());
                }
            }
        }

        names.into_iter().collect()
    })
}

/// Metadata for one installed font face.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FontFace {
    pub family: String,
    pub subfamily: String,
    pub weight: u16,
    pub style: fontdb::Style,
    pub post_script_name: String,
    pub id: fontdb::ID,
}

/// Read the font vendor's subfamily name from OpenType metadata.
#[cfg(not(target_arch = "wasm32"))]
fn face_subfamily(db: &fontdb::Database, id: fontdb::ID) -> String {
    db.with_face_data(id, |bytes, index| {
        let face = ttf_parser::Face::parse(bytes, index).ok()?;
        let names = face.names();

        for name_id in [
            ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY,
            ttf_parser::name_id::SUBFAMILY,
        ] {
            if let Some(value) = names.clone().into_iter()
                .filter(|name| name.name_id == name_id)
                .find_map(|name| name.to_string())
            {
                return Some(value);
            }
        }

        None
    })
    .flatten()
    .unwrap_or_else(|| "Regular".to_owned())
}

/// Enumerate faces using one shared cross-platform metadata model.
#[cfg(not(target_arch = "wasm32"))]
pub fn font_faces() -> &'static [FontFace] {
    static FACES: OnceLock<Vec<FontFace>> = OnceLock::new();

    FACES.get_or_init(|| {
        let db = database();
        let mut faces = Vec::new();

        for face in db.faces() {
            let Some((family, _)) = face.families.iter()
                .find(|(name, _)| !name.trim().is_empty()
                    && !name.trim_start().starts_with('.'))
            else {
                continue;
            };

            faces.push(FontFace {
                family: family.clone(),
                subfamily: face_subfamily(db, face.id),
                weight: face.weight.0,
                style: face.style,
                post_script_name: face.post_script_name.clone(),
                id: face.id,
            });
        }

        faces.sort_by(|a, b| {
            a.family.cmp(&b.family)
                .then(a.weight.cmp(&b.weight))
                .then_with(|| {
                    let rank = |style: fontdb::Style| match style {
                        fontdb::Style::Normal => 0,
                        fontdb::Style::Italic => 1,
                        fontdb::Style::Oblique => 2,
                    };
                    rank(a.style).cmp(&rank(b.style))
                })
                .then(a.subfamily.cmp(&b.subfamily))
                .then(a.post_script_name.cmp(&b.post_script_name))
        });

        faces
    })
}


/// Display a localized family name without changing its stored identity.
#[cfg(not(target_arch = "wasm32"))]
pub fn display_family_name(family: &str) -> String {
    let section = language_section(family);
    let preferred = match section {
        1 => "Chinese_Taiwan",
        2 => "Chinese_PeoplesRepublicOfChina",
        3 => "Japanese_Japan",
        4 => "Korean_SouthKorea",
        _ => return family.to_owned(),
    };

    use std::collections::HashMap;
    use std::sync::Mutex;

    static CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(value) = cache.lock().unwrap().get(family).cloned() {
        return value;
    }

    let mut result = family.to_owned();

    for face in database().faces() {
        if !face.families.iter().any(|(name, _)| name == family) {
            continue;
        }

        if let Some((name, _)) = face.families.iter()
            .find(|(_, language)| format!("{language:?}") == preferred)
        {
            result = name.clone();
            break;
        }
    }

    cache.lock().unwrap().insert(family.to_owned(), result.clone());
    result
}

/// Presentation-only grouping. Font names remain unchanged.
#[cfg(not(target_arch = "wasm32"))]
pub fn grouped_families() -> &'static [(String, Vec<String>)] {
    use std::collections::{BTreeMap, BTreeSet};

    static GROUPS: OnceLock<Vec<(String, Vec<String>)>> =
        OnceLock::new();

    GROUPS.get_or_init(|| {
        let mut groups: BTreeMap<String, BTreeSet<String>> =
            BTreeMap::new();

        for face in database().faces() {
            // Each face may have localized aliases.
            // Keep one selectable family name per face.
            let Some((name, _)) = face.families.iter().find(
                |(name, _)| {
                    !name.trim().is_empty()
                        && !name.trim_start().starts_with('.')
                }
            ) else {
                continue;
            };

            let group = font_series_name(name);

            groups
                .entry(group)
                .or_default()
                .insert(name.clone());
        }

        groups
            .into_iter()
            .map(|(group, names)| {
                (group, names.into_iter().collect())
            })
            .collect()
    })
}

/// Heuristic primary writing-system section for the font picker.
/// This is not a claim about every language supported by a font.
/// Primary script section for display only.
/// Heuristic classification; this does not represent full language coverage.
pub fn language_section(name: &str) -> usize {
    let n = name.to_lowercase();

    if [
        "pingfang sc", "kaiti sc", "songti sc",
        "heiti sc", "yuanti sc", "xingkai sc",
        "cjk sc", "noto sans sc", "noto serif sc",
    ].iter().any(|p| n.contains(p)) {
        return 2;
    }

    if [
        "pingfang", "mingliu", "dfkai", "biaukai",
        "hanwang", "kaiti tc", "songti tc",
        "heiti tc", "yuanti tc", "xingkai tc",
        "cjk tc", "cjk hk", "noto sans tc",
        "noto serif tc",
    ].iter().any(|p| n.contains(p)) {
        return 1;
    }

    if [
        "hiragino", "yu gothic", "yu mincho",
        "osaka", "cjk jp", "noto sans jp",
        "noto serif jp",
    ].iter().any(|p| n.contains(p)) {
        return 3;
    }

    if [
        "apple sd gothic", "nanum", "malgun",
        "batang", "dotum", "cjk kr",
        "noto sans kr", "noto serif kr",
    ].iter().any(|p| n.contains(p)) {
        return 4;
    }

    for (script, words) in [
        (5, &["arabic", "kufi", "naskh"][..]),
        (6, &["hebrew"][..]),
        (7, &["thai", "thonburi"][..]),
        (8, &["devanagari"][..]),
    ] {
        if words.iter().any(|word| n.contains(word)) {
            return script;
        }
    }

    if [
        "noto ", "ethiopic", "tibetan", "armenian",
        "bengali", "tamil", "telugu", "khmer",
        "lao", "myanmar", "georgian",
    ].iter().any(|p| n.contains(p)) {
        return 9;
    }

    if [
        "arial", "helvetica", "times new roman",
        "georgia", "verdana", "tahoma",
        "calibri", "aptos", "cambria",
        "courier", "consolas", "garamond",
        "trebuchet", "futura", "avenir",
        "baskerville", "palatino", "menlo",
        "monaco", "gill sans", "optima",
    ].iter().any(|p| n.starts_with(p)) {
        return 0;
    }

    // Don't mistake an English family name for Latin-script coverage.
    10
}

/// Higher-level UI series are not always encoded in font metadata.
#[cfg(not(target_arch = "wasm32"))]
fn font_series_name(name: &str) -> String {
    // Group Noto typeface series while retaining each real family.
    for (prefix, series) in [
        ("Noto Sans", "Noto Sans"),
        ("Noto Serif", "Noto Serif"),
        ("Noto Mono", "Noto Mono"),
    ] {
        if name == prefix || name.starts_with(&format!("{prefix} ")) {
            return series.to_owned();
        }
    }

    if matches!(name, "MingLiU" | "PMingLiU") {
        return "MingLiU".to_owned();
    }

    // Hiragino Pro/ProN/Std/StdN are separate real families
    // but belong together in the font picker.
    if name.starts_with("Hiragino ") {
        for suffix in [" ProN", " Pro", " StdN", " Std"] {
            if let Some(base) = name.strip_suffix(suffix) {
                return base.to_owned();
            }
        }
    }

    // Regional/language variants.
    for suffix in [" TC", " SC", " HK", " MO", "-簡", "-繁", "－簡", "－繁", "-简", "－简", " 簡", " 繁", " 简"] {
        if let Some(base) = name.strip_suffix(suffix)
            && !base.is_empty()
        {
            return base.to_owned();
        }
    }

    name.to_owned()
}

#[cfg(target_arch = "wasm32")]
pub fn grouped_families() -> &'static [(String, Vec<String>)] {
    &[]
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(name: &str) -> Option<egui::FontData> {
    use fontdb::{Family, Query, Stretch, Style, Weight};

    let db = database();

    let id = db.query(&Query { families: &[Family::Name(name)], weight: Weight::NORMAL, stretch: Stretch::Normal, style: Style::Normal })?;

    let face = db.face(id)?;

    if !face.families.iter().any(|(family, _)| family.eq_ignore_ascii_case(name)) {
        return None;
    }

    db.with_face_data(id, |bytes, index| {
        let mut font = egui::FontData::from_owned(bytes.to_vec());
        font.index = index;

        // DFKai-SB requires TrueType hinting for correct outlines.
        if name.eq_ignore_ascii_case("DFKai-SB") {
            font.tweak.hinting = Some(true);
            font.tweak.hinting_target = egui::epaint::text::HintingTarget::Mono;
        }

        font
    })
}

#[cfg(target_arch = "wasm32")]
pub fn families() -> &'static [String] {
    &[]
}

#[cfg(target_arch = "wasm32")]
pub fn load(_name: &str) -> Option<egui::FontData> {
    None
}

/// Names registered at runtime, scoped to this application's font context.
#[cfg(not(target_arch = "wasm32"))]
fn registered() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    use std::sync::{Mutex, OnceLock};
    static LOADED: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    LOADED.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_registered(name: &str) -> bool {
    registered().lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(name)
}

#[cfg(target_arch = "wasm32")]
pub fn is_registered(_name: &str) -> bool {
    false
}

/// Register a selected system font for subsequent frames.
#[cfg(not(target_arch = "wasm32"))]
pub fn register(ctx: &egui::Context, name: &str) -> bool {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

    if is_registered(name) {
        return true;
    }

    let Some(data) = load(name) else {
        return false;
    };

    let font_key = format!("installed-dynamic-{name}");
    ctx.add_font(FontInsert::new(
        &font_key,
        data,
        vec![InsertFontFamily { family: egui::FontFamily::Name(name.into()), priority: FontPriority::Highest }],
    ));

    pending().lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(name.to_string());

    ctx.request_repaint();
    true
}

#[cfg(target_arch = "wasm32")]
pub fn register(_ctx: &egui::Context, _name: &str) -> bool {
    false
}

/// Fonts requested in the current frame, not yet active.
#[cfg(not(target_arch = "wasm32"))]
fn pending() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    use std::sync::{Mutex, OnceLock};
    static PENDING: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// Activate fonts requested during the preceding frame.
#[cfg(not(target_arch = "wasm32"))]
pub fn activate_pending() {
    let names: Vec<String> = {
        let mut guard = pending().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.drain().collect()
    };

    registered().lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend(names);
}

#[cfg(target_arch = "wasm32")]
pub fn activate_pending() {}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod font_metadata_tests {
    #[test]
    fn inspect_hiragino_subfamilies() {
        let db = super::database();
        let mut faces = Vec::new();

        for face in db.faces() {
            let Some((family, _)) = face.families.first() else {
                continue;
            };
            if family != "Hiragino Sans" {
                continue;
            }

            let subfamily = db
                .with_face_data(face.id, |bytes, index| {
                    let font = ttf_parser::Face::parse(bytes, index).ok()?;
                    let names = font.names();

                    let get = |id| {
                        names.clone()
                            .into_iter()
                            .filter(|name| name.name_id == id)
                            .find_map(|name| name.to_string())
                    };

                    get(ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY)
                        .or_else(|| get(ttf_parser::name_id::SUBFAMILY))
                })
                .flatten()
                .unwrap_or_else(|| "Unknown".to_owned());

            faces.push((face.weight.0, subfamily));
        }

        faces.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        for (weight, subfamily) in &faces {
            println!("{subfamily} (weight: {weight})");
        }

        assert!(!faces.is_empty(), "No Hiragino Sans fonts found");
        assert!(faces.windows(2).all(|w| w[0].0 <= w[1].0));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod font_face_model_tests {
    #[test]
    fn inspect_font_face_model() {
        let faces = super::font_faces();

        for family in ["Arial", "PingFang TC", "Hiragino Sans"] {
            println!("\n=== {family} ===");

            for face in faces.iter().filter(|f| f.family == family) {
                println!(
                    "{} | {} | weight {}",
                    face.family,
                    face.subfamily,
                    face.weight
                );
            }
        }

        assert!(!faces.is_empty(), "No installed fonts discovered");
        assert!(faces.windows(2).all(|pair| {
            pair[0].family < pair[1].family
                || (pair[0].family == pair[1].family
                    && pair[0].weight <= pair[1].weight)
        }));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod localized_font_name_tests {
    #[test]
    fn inspect_localized_family_names() {
        let db = super::database();

        for target in [
            "DFKai-SB",
            "PingFang TC",
            "Hiragino Sans",
            "Arial",
        ] {
            println!("\n=== {target} ===");

            let mut found = false;

            for face in db.faces() {
                if !face.families.iter().any(|(name, _)| name == target) {
                    continue;
                }

                if found {
                    continue;
                }

                found = true;

                for (name, language) in &face.families {
                    println!("  {:?} => {}", language, name);
                }
            }

            if !found {
                println!("  Not installed or not found");
            }
        }
    }
}
