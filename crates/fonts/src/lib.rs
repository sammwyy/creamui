//! Global font registry: register font files, then resolve a CSS-style
//! family stack (`"Inter, sans-serif"`) to a loaded face.

mod face;
mod layout;

pub use face::{FontFace, LineMetrics};
pub use layout::{
    layout, CharPosition, HorizontalAlign, LayoutSettings, Line, PositionedGlyph, TextLayout,
};

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;
use std::{env, fs};

/// Generic UI family resolved from the operating system on first use.
///
/// CreamUI deliberately does not bundle a font: shipping one makes every
/// executable larger, while system fonts are already on disk and are
/// memory-mapped rather than copied.
pub const DEFAULT_FAMILY: &str = "system-ui";

/// Embeds a font file's bytes at compile time.
#[macro_export]
macro_rules! include_font {
    ($path:literal) => {
        include_bytes!($path)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    Bold,
}

#[derive(Debug)]
pub struct FontError(String);

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "creamui-fonts: {}", self.0)
    }
}
impl std::error::Error for FontError {}

struct Registry {
    faces: HashMap<(String, FontWeight), Rc<FontFace>>,
}

impl Registry {
    fn with_defaults() -> Self {
        Registry {
            faces: HashMap::new(),
        }
    }
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::with_defaults());
    static PREFERRED_FAMILY: RefCell<Option<String>> = RefCell::new(None);
}

/// Registers `bytes` as `family`'s face for `weight`, replacing any face
/// previously registered for that (family, weight) pair.
pub fn register_bytes(
    family: impl Into<String>,
    weight: FontWeight,
    bytes: impl AsRef<[u8]>,
) -> Result<(), FontError> {
    register_face(family.into(), weight, FontFace::from_bytes(bytes.as_ref())?);
    Ok(())
}

/// Memory-maps `path` and registers it.
pub fn register_file(
    family: impl Into<String>,
    weight: FontWeight,
    path: impl AsRef<Path>,
) -> std::io::Result<()> {
    let face = FontFace::from_path(path.as_ref())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    register_face(family.into(), weight, face);
    Ok(())
}

fn register_face(family: String, weight: FontWeight, face: FontFace) {
    REGISTRY.with(|registry| {
        registry
            .borrow_mut()
            .faces
            .insert((family, weight), Rc::new(face));
    });
}

/// Resolves a CSS-style comma-separated family stack against the registry and
/// the operating system. Faces are parsed only when their family and weight
/// are first requested. A `Bold` request falls back to its family's regular
/// face before trying [`DEFAULT_FAMILY`].
pub fn resolve(spec: &str, weight: FontWeight) -> Rc<FontFace> {
    let families: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .collect();
    for family in &families {
        if let Some(face) = registered_face(family, weight) {
            return face;
        }
        if is_generic_family(family) {
            continue;
        }
        if load_system_face(family, family, weight) {
            return registered_face(family, weight)
                .expect("a successfully loaded system face is registered");
        }
        if weight == FontWeight::Bold {
            if let Some(face) = registered_face(family, FontWeight::Regular) {
                return face;
            }
        }
    }
    default_face(weight)
}

fn registered_face(family: &str, weight: FontWeight) -> Option<Rc<FontFace>> {
    REGISTRY.with(|registry| {
        registry
            .borrow()
            .faces
            .get(&(family.to_owned(), weight))
            .cloned()
    })
}

fn default_face(weight: FontWeight) -> Rc<FontFace> {
    let weights: &[FontWeight] = match weight {
        FontWeight::Regular => &[FontWeight::Regular],
        FontWeight::Bold => &[FontWeight::Bold, FontWeight::Regular],
    };
    for &weight in weights {
        if let Some(face) = registered_face(DEFAULT_FAMILY, weight) {
            return face;
        }
        for family in system_font_candidates() {
            if load_system_face(DEFAULT_FAMILY, family, weight) {
                return registered_face(DEFAULT_FAMILY, weight)
                    .expect("a successfully loaded system face is registered");
            }
        }
    }
    panic!(
        "creamui-fonts: no usable system UI font was found; register one with register_file/register_bytes"
    );
}

fn load_system_face(registry_family: &str, lookup_family: &str, weight: FontWeight) -> bool {
    let Some(path) = find_system_font(lookup_family, weight) else {
        return false;
    };
    match FontFace::from_path(&path) {
        Ok(face) => {
            log::debug!(
                "creamui-fonts: mapped {} as {registry_family} {weight:?}",
                path.display()
            );
            register_face(registry_family.to_owned(), weight, face);
            true
        }
        Err(err) => {
            log::warn!("creamui-fonts: skipping {err}");
            false
        }
    }
}

fn is_generic_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "system-ui" | "sans-serif" | "serif" | "monospace"
    )
}

#[cfg(target_os = "windows")]
const SYSTEM_FONT_CANDIDATES: &[&str] = &["Segoe UI", "Arial"];
#[cfg(target_os = "macos")]
const SYSTEM_FONT_CANDIDATES: &[&str] = &["Helvetica Neue", "Helvetica", "Arial"];
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const SYSTEM_FONT_CANDIDATES: &[&str] = &["Liberation Sans", "DejaVu Sans", "Noto Sans", "Arial"];

fn system_font_candidates() -> &'static [&'static str] {
    SYSTEM_FONT_CANDIDATES
}

/// The family stack [`resolve`] and [`use_font`] fall back to when no
/// family is explicitly requested, following the last font loaded with
/// [`use_system_font`] (or [`DEFAULT_FAMILY`] otherwise).
pub fn preferred_family() -> String {
    PREFERRED_FAMILY
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| DEFAULT_FAMILY.to_string())
}

/// Sets the family [`preferred_family`] returns, without touching the
/// registry.
pub fn set_preferred_family(family: Option<String>) {
    PREFERRED_FAMILY.with(|cell| *cell.borrow_mut() = family);
}

/// Loads `family` from the system's installed fonts by matching a font
/// file's name against it (ignoring case, spaces, and dashes), registers
/// it, and makes it the new [`preferred_family`]. Leaves the registry
/// and the preferred family untouched and returns `false` when no
/// matching font file is found on disk.
pub fn use_system_font(family: &str) -> bool {
    if !load_system_face(family, family, FontWeight::Regular) {
        return false;
    }
    let _ = load_system_face(family, family, FontWeight::Bold);
    set_preferred_family(Some(family.to_owned()));
    true
}

fn find_system_font(family: &str, weight: FontWeight) -> Option<PathBuf> {
    match_font(system_font_index(), family, weight)
}

fn match_font(index: &[(String, PathBuf)], family: &str, weight: FontWeight) -> Option<PathBuf> {
    let target = normalize_font_name(family);
    let matches: Vec<&(String, PathBuf)> = index
        .iter()
        .filter(|(stem, _)| stem.starts_with(&target))
        .collect();
    let wants_bold = weight == FontWeight::Bold;
    let preferred_stems = if wants_bold {
        [format!("{target}bold"), format!("{target}semibold")]
    } else {
        [target.clone(), format!("{target}regular")]
    };
    preferred_stems
        .iter()
        .find_map(|preferred| {
            matches
                .iter()
                .find(|(stem, _)| stem == preferred)
                .map(|(_, path)| path.clone())
        })
        .or_else(|| {
            matches
                .iter()
                .find(|(stem, _)| stem.contains("bold") == wants_bold)
                .map(|(_, path)| path.clone())
        })
        .or_else(|| {
            (!wants_bold)
                .then(|| matches.first())
                .flatten()
                .map(|(_, path)| path.clone())
        })
}

fn normalize_font_name(name: &str) -> String {
    name.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn system_font_index() -> &'static [(String, PathBuf)] {
    static INDEX: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut entries = Vec::new();
        for directory in system_font_directories() {
            collect_font_files(&directory, &mut entries);
        }
        entries
    })
}

fn system_font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = env::var_os("WINDIR") {
            directories.push(PathBuf::from(windir).join("Fonts"));
        }
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            directories.push(PathBuf::from(local_app_data).join("Microsoft/Windows/Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    directories.extend([
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ]);
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    directories.extend([
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ]);
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.push(home.join(".local/share/fonts"));
        directories.push(home.join(".fonts"));
        #[cfg(target_os = "macos")]
        directories.push(home.join("Library/Fonts"));
    }
    directories
}

fn collect_font_files(directory: &Path, entries: &mut Vec<(String, PathBuf)>) {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_font_files(&path, entries);
            continue;
        }
        let is_font = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "ttf" | "otf" | "ttc"
                )
            });
        let Some(stem) = is_font.then(|| path.file_stem()).flatten() else {
            continue;
        };
        if let Some(stem) = stem.to_str() {
            entries.push((normalize_font_name(stem), path));
        }
    }
}

/// Both weights of a resolved family stack.
#[derive(Clone)]
pub struct FontHandle {
    pub regular: Rc<FontFace>,
    pub bold: Rc<FontFace>,
}

impl FontHandle {
    pub fn weight(&self, weight: FontWeight) -> &Rc<FontFace> {
        match weight {
            FontWeight::Regular => &self.regular,
            FontWeight::Bold => &self.bold,
        }
    }
}

/// Resolves `spec` against the registry. Only callable while a
/// `with_context_scope` is active (e.g. during a window's `build_ui`);
/// panics otherwise.
pub fn use_font(spec: impl AsRef<str>) -> FontHandle {
    creamui_reactive::require_context_scope("use_font");
    FontHandle {
        regular: resolve(spec.as_ref(), FontWeight::Regular),
        bold: resolve(spec.as_ref(), FontWeight::Bold),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font_bytes() -> Vec<u8> {
        let path = system_font_candidates()
            .iter()
            .find_map(|family| find_system_font(family, FontWeight::Regular))
            .expect("tests need one of the supported system UI fonts");
        fs::read(path).expect("system UI font remains readable")
    }

    #[test]
    fn resolves_the_system_default_family() {
        let face = resolve(DEFAULT_FAMILY, FontWeight::Regular);
        assert!(face.glyph_count() > 0);
    }

    #[test]
    fn unknown_family_falls_back_to_the_default() {
        let default = resolve(DEFAULT_FAMILY, FontWeight::Regular);
        let fallback = resolve("Nonexistent Family", FontWeight::Regular);
        assert!(Rc::ptr_eq(&default, &fallback));
    }

    #[test]
    fn css_style_stack_resolves_to_the_first_registered_family() {
        register_bytes("Test Family A", FontWeight::Regular, test_font_bytes()).unwrap();
        let resolved = resolve(
            "Nonexistent, Test Family A, sans-serif",
            FontWeight::Regular,
        );
        let expected = resolve("Test Family A", FontWeight::Regular);
        assert!(Rc::ptr_eq(&resolved, &expected));
    }

    #[test]
    fn bold_falls_back_to_the_family_s_own_regular_before_the_default() {
        register_bytes("Test Family B", FontWeight::Regular, test_font_bytes()).unwrap();
        let own_regular = resolve("Test Family B", FontWeight::Regular);
        let bold_request = resolve("Test Family B", FontWeight::Bold);
        assert!(Rc::ptr_eq(&own_regular, &bold_request));
    }

    #[test]
    fn bold_loads_its_own_system_face_after_the_regular_one() {
        let regular = resolve(DEFAULT_FAMILY, FontWeight::Regular);
        let bold = resolve(DEFAULT_FAMILY, FontWeight::Bold);
        assert!(!Rc::ptr_eq(&regular, &bold));
    }

    #[test]
    fn register_bytes_rejects_invalid_font_data() {
        assert!(register_bytes("Broken", FontWeight::Regular, b"not a font").is_err());
    }

    #[test]
    #[should_panic(expected = "use_font")]
    fn use_font_outside_a_context_scope_panics() {
        use_font("sans-serif");
    }

    #[test]
    fn use_font_inside_a_context_scope_resolves() {
        creamui_reactive::with_context_scope(|| {
            let handle = use_font(DEFAULT_FAMILY);
            assert!(handle.regular.glyph_count() > 0);
            assert!(handle.bold.glyph_count() > 0);
        });
    }

    #[test]
    fn preferred_family_defaults_to_the_system_family() {
        set_preferred_family(None);
        assert_eq!(preferred_family(), DEFAULT_FAMILY);
        set_preferred_family(Some("Inter".to_owned()));
        assert_eq!(preferred_family(), "Inter");
        set_preferred_family(None);
    }

    #[test]
    fn normalizes_case_spaces_and_dashes() {
        assert_eq!(normalize_font_name("Fira Code"), "firacode");
        assert_eq!(normalize_font_name("Fira-Code-Bold"), "firacodebold");
    }

    #[test]
    fn matches_a_regular_face_over_a_bold_one_with_the_same_stem() {
        let index = vec![
            (
                normalize_font_name("Inter-Bold"),
                PathBuf::from("/fonts/Inter-Bold.ttf"),
            ),
            (
                normalize_font_name("Inter-Regular"),
                PathBuf::from("/fonts/Inter-Regular.ttf"),
            ),
        ];
        assert_eq!(
            match_font(&index, "Inter", FontWeight::Regular),
            Some(PathBuf::from("/fonts/Inter-Regular.ttf"))
        );
        assert_eq!(
            match_font(&index, "Inter", FontWeight::Bold),
            Some(PathBuf::from("/fonts/Inter-Bold.ttf"))
        );
    }

    #[test]
    fn prefers_an_exact_latin_family_over_a_prefixed_cjk_variant() {
        let index = vec![
            (
                normalize_font_name("NotoSansCJK-Regular"),
                PathBuf::from("/fonts/NotoSansCJK-Regular.ttf"),
            ),
            (
                normalize_font_name("NotoSans-Regular"),
                PathBuf::from("/fonts/NotoSans-Regular.ttf"),
            ),
        ];
        assert_eq!(
            match_font(&index, "Noto Sans", FontWeight::Regular),
            Some(PathBuf::from("/fonts/NotoSans-Regular.ttf"))
        );
    }

    #[test]
    fn falls_back_to_any_match_when_a_family_has_no_plain_regular_file() {
        let index = vec![(
            normalize_font_name("Inter-Bold"),
            PathBuf::from("/fonts/Inter-Bold.ttf"),
        )];
        assert_eq!(
            match_font(&index, "Inter", FontWeight::Regular),
            Some(PathBuf::from("/fonts/Inter-Bold.ttf"))
        );
    }

    #[test]
    fn a_missing_bold_variant_matches_nothing() {
        let index = vec![(
            normalize_font_name("Inter-Regular"),
            PathBuf::from("/fonts/Inter-Regular.ttf"),
        )];
        assert_eq!(match_font(&index, "Inter", FontWeight::Bold), None);
    }

    #[test]
    fn unrelated_families_never_match() {
        let index = vec![(
            normalize_font_name("Inter-Regular"),
            PathBuf::from("/fonts/Inter-Regular.ttf"),
        )];
        assert_eq!(match_font(&index, "Roboto", FontWeight::Regular), None);
    }

    #[test]
    fn collects_only_font_files_recursively() {
        let root =
            std::env::temp_dir().join(format!("creamui-fonts-collect-{}", std::process::id()));
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("Sample.ttf"), b"not a real font").unwrap();
        fs::write(nested.join("Sample.otf"), b"not a real font").unwrap();
        fs::write(root.join("notes.txt"), b"ignore me").unwrap();

        let mut entries = Vec::new();
        collect_font_files(&root, &mut entries);
        entries.sort();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|(stem, _)| stem == "sample"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn use_system_font_leaves_the_preferred_family_unset_when_nothing_matches() {
        set_preferred_family(None);
        assert!(!use_system_font("Definitely Not An Installed Family XYZ"));
        assert_eq!(preferred_family(), DEFAULT_FAMILY);
    }
}
