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

        for name_id in [ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY, ttf_parser::name_id::SUBFAMILY] {
            if let Some(value) = names.clone().into_iter().filter(|name| name.name_id == name_id).find_map(|name| name.to_string()) {
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
            let Some((family, _)) = face.families.iter().find(|(name, _)| !name.trim().is_empty() && !name.trim_start().starts_with('.')) else {
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
            a.family
                .cmp(&b.family)
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

/// A font family containing its installed faces.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FontFamilyGroup {
    pub family: String,
    pub display_name: String,
    pub section: usize,
    pub faces: Vec<FontFace>,
}

/// Group installed faces by family, preserving weight and style order.
#[cfg(not(target_arch = "wasm32"))]
pub fn font_family_groups() -> &'static [FontFamilyGroup] {
    static GROUPS: OnceLock<Vec<FontFamilyGroup>> = OnceLock::new();

    GROUPS.get_or_init(|| {
        use std::collections::BTreeMap;

        let mut by_family: BTreeMap<String, Vec<FontFace>> = BTreeMap::new();

        for face in font_faces() {
            by_family.entry(face.family.clone()).or_default().push(face.clone());
        }

        let mut groups: Vec<FontFamilyGroup> = by_family
            .into_iter()
            .map(|(family, faces)| FontFamilyGroup { display_name: display_family_name(&family), section: language_section(&family), family, faces })
            .collect();

        groups.sort_by(|a, b| a.section.cmp(&b.section).then(a.display_name.cmp(&b.display_name)).then(a.family.cmp(&b.family)));

        groups
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

        if let Some((name, _)) = face.families.iter().find(|(_, language)| format!("{language:?}") == preferred) {
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

    static GROUPS: OnceLock<Vec<(String, Vec<String>)>> = OnceLock::new();

    GROUPS.get_or_init(|| {
        let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for face in database().faces() {
            // Each face may have localized aliases.
            // Keep one selectable family name per face.
            let Some((name, _)) = face.families.iter().find(|(name, _)| !name.trim().is_empty() && !name.trim_start().starts_with('.')) else {
                continue;
            };

            let group = font_series_name(name);

            groups.entry(group).or_default().insert(name.clone());
        }

        groups.into_iter().map(|(group, names)| (group, names.into_iter().collect())).collect()
    })
}

/// Heuristic primary writing-system section for the font picker.
/// This is not a claim about every language supported by a font.
/// Primary script section for display only.
/// Heuristic classification; this does not represent full language coverage.
pub fn language_section(name: &str) -> usize {
    let n = name.to_lowercase();

    if ["pingfang sc", "kaiti sc", "songti sc", "heiti sc", "yuanti sc", "xingkai sc", "cjk sc", "noto sans sc", "noto serif sc"]
        .iter()
        .any(|p| n.contains(p))
    {
        return 2;
    }

    if [
        "pingfang",
        "mingliu",
        "dfkai",
        "biaukai",
        "hanwang",
        "kaiti tc",
        "songti tc",
        "heiti tc",
        "yuanti tc",
        "xingkai tc",
        "cjk tc",
        "cjk hk",
        "noto sans tc",
        "noto serif tc",
    ]
    .iter()
    .any(|p| n.contains(p))
    {
        return 1;
    }

    if ["hiragino", "yu gothic", "yu mincho", "osaka", "cjk jp", "noto sans jp", "noto serif jp"].iter().any(|p| n.contains(p)) {
        return 3;
    }

    if ["apple sd gothic", "nanum", "malgun", "batang", "dotum", "cjk kr", "noto sans kr", "noto serif kr"].iter().any(|p| n.contains(p)) {
        return 4;
    }

    for (script, words) in [(5, &["arabic", "kufi", "naskh"][..]), (6, &["hebrew"][..]), (7, &["thai", "thonburi"][..]), (8, &["devanagari"][..])] {
        if words.iter().any(|word| n.contains(word)) {
            return script;
        }
    }

    if ["noto ", "ethiopic", "tibetan", "armenian", "bengali", "tamil", "telugu", "khmer", "lao", "myanmar", "georgian"].iter().any(|p| n.contains(p))
    {
        return 9;
    }

    if [
        "arial",
        "helvetica",
        "times new roman",
        "georgia",
        "verdana",
        "tahoma",
        "calibri",
        "aptos",
        "cambria",
        "courier",
        "consolas",
        "garamond",
        "trebuchet",
        "futura",
        "avenir",
        "baskerville",
        "palatino",
        "menlo",
        "monaco",
        "gill sans",
        "optima",
    ]
    .iter()
    .any(|p| n.starts_with(p))
    {
        return 0;
    }

    // Don't mistake an English family name for Latin-script coverage.
    10
}

/// Higher-level UI series are not always encoded in font metadata.
#[cfg(not(target_arch = "wasm32"))]
fn font_series_name(name: &str) -> String {
    // Group Noto typeface series while retaining each real family.
    for (prefix, series) in [("Noto Sans", "Noto Sans"), ("Noto Serif", "Noto Serif"), ("Noto Mono", "Noto Mono")] {
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

/// Runtime egui family key for a specific installed face.
/// This key must never be stored in the document.
#[cfg(not(target_arch = "wasm32"))]
pub fn face_family_key(family: &str, weight: u16, style: fontdb::Style) -> String {
    let style = match style {
        fontdb::Style::Normal => "normal",
        fontdb::Style::Italic => "italic",
        fontdb::Style::Oblique => "oblique",
    };
    format!("installed-face:{family}:{weight}:{style}")
}

/// Find an installed face by its stable font properties.
/// Returns None rather than silently substituting another weight.
#[cfg(not(target_arch = "wasm32"))]
pub fn find_font_face(family: &str, weight: u16, style: fontdb::Style) -> Option<&'static FontFace> {
    font_faces().iter().find(|face| face.family == family && face.weight == weight && face.style == style)
}

/// Load the exact installed face, including its collection index.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_face(face: &FontFace) -> Option<egui::FontData> {
    let db = database();

    // Face IDs are valid only for this process's shared font database.
    let installed = db.face(face.id)?;

    if installed.post_script_name != face.post_script_name || !installed.families.iter().any(|(name, _)| name == &face.family) {
        return None;
    }

    db.with_face_data(face.id, |bytes, index| {
        let mut font = egui::FontData::from_owned(bytes.to_vec());
        font.index = index;

        // Preserve the existing special rendering fix.
        if face.family.eq_ignore_ascii_case("DFKai-SB") {
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

/// Register an exact font face for use in subsequent frames.
/// Returns false when the requested face is not installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn register_face(ctx: &egui::Context, family: &str, weight: u16, style: fontdb::Style) -> bool {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

    let key = face_family_key(family, weight, style);

    if is_registered(&key) {
        return true;
    }

    if pending().lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&key) {
        return false;
    }

    let Some(face) = find_font_face(family, weight, style) else {
        return false;
    };
    let Some(data) = load_face(face) else {
        return false;
    };

    ctx.add_font(FontInsert::new(
        &key,
        data,
        vec![InsertFontFamily { family: egui::FontFamily::Name(key.clone().into()), priority: FontPriority::Highest }],
    ));

    pending().lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key);

    ctx.request_repaint();
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

// TODO: Some installed families expose distinct faces with identical
// weight and style metadata (e.g. Yuanti TC Regular/Bold/Light all 400).
// Family + Weight + Style cannot identify those faces uniquely.
// Preserve this known limitation until exact face identity is supported.

#[cfg(all(test, not(target_arch = "wasm32")))]
mod font_selection_tests {
    use super::*;

    #[test]
    fn families_and_faces_are_ordered() {
        let groups = font_family_groups();

        assert!(
            groups.windows(2).all(|pair| {
                (pair[0].section, &pair[0].display_name, &pair[0].family) <= (pair[1].section, &pair[1].display_name, &pair[1].family)
            })
        );

        for group in groups {
            assert!(!group.faces.is_empty());
            assert!(group.faces.iter().all(|face| face.family == group.family));
            assert!(group.faces.windows(2).all(|pair| { pair[0].weight <= pair[1].weight }));
        }
    }

    #[test]
    fn exact_face_lookup_and_loading() {
        // Avoid assuming that any specific font is installed.
        for face in font_faces().iter().take(10) {
            let matches = font_faces()
                .iter()
                .filter(|candidate| candidate.family == face.family && candidate.weight == face.weight && candidate.style == face.style)
                .count();

            // Skip ambiguous metadata: different faces can share
            // Family + Weight + Style.
            if matches != 1 {
                continue;
            }

            let found = find_font_face(&face.family, face.weight, face.style).expect("Installed face should be found");

            assert_eq!(found.id, face.id);
            assert!(load_face(found).is_some(), "Unable to load installed face: {}", face.family);
        }
    }
}
