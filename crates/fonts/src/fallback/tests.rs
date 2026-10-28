use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
use swash::text::{
    analyze,
    cluster::{CharInfo, Parser, Token},
    Script,
};

const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../demo/showcase/assets/LiberationSans-Regular.ttf"
));

fn bytes(chars: &[char], symbol: bool) -> Vec<u8> {
    let mut font = FONT.to_vec();
    let count = u16::from_be_bytes(font[4..6].try_into().unwrap()) as usize;
    let offset = (0..count)
        .find_map(|index| {
            let record = 12 + index * 16;
            (&font[record..record + 4] == b"cmap").then(|| {
                u32::from_be_bytes(font[record + 8..record + 12].try_into().unwrap()) as usize
            })
        })
        .unwrap();
    let mut points: Vec<_> = chars.iter().map(|ch| *ch as u32).collect();
    points.sort_unstable();
    points.dedup();
    let mut cmap = Vec::new();
    for value in [0_u16, 1, 3, if symbol { 0 } else { 10 }] {
        cmap.extend_from_slice(&value.to_be_bytes());
    }
    cmap.extend_from_slice(&12_u32.to_be_bytes());
    cmap.extend_from_slice(&12_u16.to_be_bytes());
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&(16 + points.len() as u32 * 12).to_be_bytes());
    cmap.extend_from_slice(&0_u32.to_be_bytes());
    cmap.extend_from_slice(&(points.len() as u32).to_be_bytes());
    for point in points {
        for value in [point, point, 36] {
            cmap.extend_from_slice(&value.to_be_bytes());
        }
    }
    font[offset..offset + cmap.len()].copy_from_slice(&cmap);
    font
}

fn cluster(text: &str) -> CharCluster {
    let tokens = text.char_indices().zip(analyze(text.chars())).map(
        |((offset, ch), (properties, boundary))| Token {
            ch,
            offset: offset as u32,
            len: ch.len_utf8() as u8,
            info: CharInfo::new(properties, boundary),
            data: 0,
        },
    );
    let mut result = CharCluster::new();
    assert!(Parser::new(Script::Latin, tokens).next(&mut result));
    result
}

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "creamui-fallback-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn font(&self, name: &str, chars: &[char]) -> PathBuf {
        let path = self.0.join(format!("{name}.ttf"));
        fs::write(&path, bytes(chars, false)).unwrap();
        path
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn coverage_matches_real_mapping_including_symbol_aliases_and_normalization() {
    for (chars, symbol) in [
        (vec!['A', 'B', '\u{301}', '\u{c1}', '\u{1f600}'], false),
        (vec!['\u{f041}'], true),
    ] {
        let face = FontFace::from_bytes(&bytes(&chars, symbol)).unwrap();
        let coverage = Coverage::new(&face);
        let font = face.font_ref();
        let charmap = font.charmap();
        if symbol {
            assert_ne!(charmap.map('A'), 0);
        }
        for codepoint in 0..=0x10ffff {
            if let Some(ch) = char::from_u32(codepoint) {
                assert_eq!(coverage.contains(ch), charmap.map(ch) != 0, "{codepoint:x}");
            }
        }
        for text in ["A", "A\u{301}", "\u{1f600}", "\u{10ffff}"] {
            let mut actual = cluster(text);
            assert_eq!(
                coverage.improves(&actual),
                actual.map(|ch| charmap.map(ch)) != Status::Discard
            );
        }
    }
}

#[test]
fn rejected_files_keep_only_coverage_and_do_not_reopen_for_other_missing_characters() {
    let files = Files::new();
    let path = files.font("latin", &['A']);
    let mut cache = FallbackCache::default();
    assert!(cache
        .candidate(&path, &cluster("\u{10ffff}"), FontFace::from_path)
        .is_none());
    assert_eq!(cache.files.len(), 0);
    assert_eq!(cache.coverage.len(), 1);
    assert!(cache
        .candidate(&path, &cluster("\u{ffff}"), |_| panic!(
            "rejected coverage must avoid reopening the file"
        ))
        .is_none());
    let face = cache
        .candidate(&path, &cluster("A"), FontFace::from_path)
        .unwrap();
    assert_ne!(face.font_ref().charmap().map('A'), 0);
}

#[test]
fn file_eviction_does_not_invalidate_borrowers_and_character_keys_are_weak() {
    let files = Files::new();
    let first = files.font("first", &['A']);
    let second = files.font("second", &['B']);
    let mut cache = FallbackCache {
        files: Lru::new(1, MAX_FILE_BYTES),
        ..FallbackCache::default()
    };
    let first_face = cache
        .candidate(&first, &cluster("A"), FontFace::from_path)
        .unwrap();
    let weak = Rc::downgrade(&first_face);
    cache.remember(Some((1, 1, 'A')), Some(first_face.clone()), Some(&first));
    let second_face = cache
        .candidate(&second, &cluster("B"), FontFace::from_path)
        .unwrap();
    cache.remember(Some((1, 1, 'B')), Some(second_face), Some(&second));
    assert_eq!(cache.files.len(), 1);
    assert_ne!(first_face.font_ref().charmap().map('A'), 0);
    drop(first_face);
    assert!(weak.upgrade().is_none());
    assert!(cache.character(&(1, 1, 'A')).is_none());
    assert!(cache.character(&(1, 1, 'B')).unwrap().is_some());
}

#[test]
fn oversized_selected_files_are_returned_without_flushing_or_bypassing_the_budget() {
    let files = Files::new();
    let keep = files.font("keep", &['A']);
    let huge = files.font("huge", &['B']);
    fs::OpenOptions::new()
        .write(true)
        .open(&huge)
        .unwrap()
        .set_len(MAX_FILE_BYTES as u64 + 1)
        .unwrap();
    let mut cache = FallbackCache::default();
    let keep_face = cache
        .candidate(&keep, &cluster("A"), FontFace::from_path)
        .unwrap();
    cache.remember(Some((1, 1, 'A')), Some(keep_face), Some(&keep));
    let huge_face = cache
        .candidate(&huge, &cluster("B"), FontFace::from_path)
        .unwrap();
    let weak = Rc::downgrade(&huge_face);
    let huge_face = cache
        .remember(Some((1, 1, 'B')), Some(huge_face), Some(&huge))
        .unwrap();
    assert_eq!(cache.files.len(), 1);
    assert!(cache.files.bytes() <= MAX_FILE_BYTES);
    assert!(cache.character(&(1, 1, 'A')).unwrap().is_some());
    drop(huge_face);
    assert!(weak.upgrade().is_none());
}

#[test]
fn all_cache_budgets_include_negative_entries_and_release_evicted_values() {
    let mut cache = FallbackCache::default();
    for index in 0..MAX_CHARACTERS + 100 {
        cache.remember(Some((index as u64, 0, '\u{10ffff}')), None, None);
    }
    assert_eq!(cache.chars.len(), MAX_CHARACTERS);
    cache.coverage = Lru::new(3, 256);
    for index in 0..100 {
        cache.remember_coverage(Path::new(&format!("font-{index}")), None);
    }
    assert!(cache.coverage.len() <= 3);
    assert!(cache.coverage.bytes() <= 256);
    let mut values = Lru::new(10, 10);
    values.insert(1, Rc::new(1), 7);
    let weak = Rc::downgrade(values.get(&1).unwrap());
    values.insert(2, Rc::new(2), 7);
    assert!(weak.upgrade().is_none());
    assert_eq!(values.bytes(), 7);
}

#[test]
fn negative_characters_are_invalidated_when_a_covering_face_is_registered() {
    CACHE.with(|cache| *cache.borrow_mut() = FallbackCache::default());
    let primary = FontFace::from_bytes(&bytes(&['A'], false)).unwrap();
    assert!(fallback_with_index(&primary, &mut cluster("\u{10ffff}"), || &[]).is_none());
    assert!(
        fallback_with_index(&primary, &mut cluster("\u{10ffff}"), || panic!(
            "cached negative must avoid the index"
        ))
        .is_none()
    );
    crate::register_bytes(
        "Fallback Invalidation",
        crate::FontWeight::Regular,
        bytes(&['\u{10ffff}'], false),
    )
    .unwrap();
    let face = fallback_with_index(&primary, &mut cluster("\u{10ffff}"), || {
        panic!("registered coverage must avoid the index")
    })
    .unwrap();
    assert_ne!(face.font_ref().charmap().map('\u{10ffff}'), 0);
}

#[test]
fn normalized_complete_matches_replace_partial_choices_in_index_order() {
    CACHE.with(|cache| *cache.borrow_mut() = FallbackCache::default());
    let files = Files::new();
    let partial = files.font("partial", &['A']);
    let complete = files.font("complete", &['\u{c1}']);
    let index = vec![
        ("partial".into(), partial),
        ("complete".into(), complete.clone()),
    ];
    let primary = FontFace::from_bytes(&bytes(&['X'], false)).unwrap();
    let mut cluster = cluster("A\u{301}");
    let face = fallback_with_index(&primary, &mut cluster, || &index).unwrap();
    assert_ne!(face.font_ref().charmap().map('\u{c1}'), 0);
    assert!(cluster.mapped_chars().iter().all(|ch| ch.glyph_id != 0));
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        assert_eq!(cache.files.len(), 1);
        assert!(Rc::ptr_eq(cache.files.get(&complete).unwrap(), &face));
    });
}
