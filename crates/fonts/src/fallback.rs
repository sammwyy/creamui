mod cache;
mod coverage;

use std::{
    cell::RefCell,
    mem::size_of,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};
use swash::text::cluster::{CharCluster, Status};

use crate::{registry_generation, system_font_index, FontError, FontFace, REGISTRY};
use cache::Lru;
use coverage::Coverage;

const MAX_FILES: usize = 32;
const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_COVERAGES: usize = 1024;
const MAX_COVERAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CHARACTERS: usize = 4096;

type CharacterKey = (u64, u64, char);

struct FallbackCache {
    files: Lru<PathBuf, Rc<FontFace>>,
    coverage: Lru<PathBuf, Option<Coverage>>,
    chars: Lru<CharacterKey, Option<Weak<FontFace>>>,
}

impl Default for FallbackCache {
    fn default() -> Self {
        Self {
            files: Lru::new(MAX_FILES, MAX_FILE_BYTES),
            coverage: Lru::new(MAX_COVERAGES, MAX_COVERAGE_BYTES),
            chars: Lru::new(MAX_CHARACTERS, usize::MAX),
        }
    }
}

thread_local! {
    static CACHE: RefCell<FallbackCache> = RefCell::new(FallbackCache::default());
}

impl FallbackCache {
    fn remember_coverage(&mut self, path: &Path, coverage: Option<Coverage>) {
        let path = path.to_path_buf();
        let bytes = path.capacity()
            + size_of::<Option<Coverage>>()
            + coverage.as_ref().map_or(0, Coverage::bytes);
        self.coverage.insert(path, coverage, bytes);
    }

    fn character(&mut self, key: &CharacterKey) -> Option<Option<Rc<FontFace>>> {
        let cached = self.chars.get(key)?;
        match cached {
            Some(face) => {
                let face = face.upgrade()?;
                self.files.touch(|cached| Rc::ptr_eq(cached, &face));
                Some(Some(face))
            }
            None => Some(None),
        }
    }

    fn candidate(
        &mut self,
        path: &Path,
        cluster: &CharCluster,
        load: impl FnOnce(&Path) -> Result<FontFace, FontError>,
    ) -> Option<Rc<FontFace>> {
        let needs_coverage = match self.coverage.get(path) {
            Some(coverage)
                if !coverage
                    .as_ref()
                    .is_some_and(|coverage| coverage.improves(cluster)) =>
            {
                return None
            }
            Some(_) => false,
            None => true,
        };
        let face = match self.files.get(path).cloned() {
            Some(face) => face,
            None => match load(path) {
                Ok(face) => Rc::new(face),
                Err(error) => {
                    log::debug!("creamui-fonts: skipping fallback {error}");
                    self.remember_coverage(path, None);
                    return None;
                }
            },
        };
        if needs_coverage {
            let coverage = Coverage::new(&face);
            let improves = coverage.improves(cluster);
            self.remember_coverage(path, Some(coverage));
            if !improves {
                return None;
            }
        }
        Some(face)
    }

    fn remember(
        &mut self,
        key: Option<CharacterKey>,
        face: Option<Rc<FontFace>>,
        path: Option<&Path>,
    ) -> Option<Rc<FontFace>> {
        if let (Some(face), Some(path)) = (&face, path) {
            if !self
                .files
                .get(path)
                .is_some_and(|cached| Rc::ptr_eq(cached, face))
            {
                let path = path.to_path_buf();
                let bytes = face.font_ref().data.len().saturating_add(path.capacity());
                self.files.insert(path, face.clone(), bytes);
            }
        }
        if let Some(key) = key {
            self.chars.insert(key, face.as_ref().map(Rc::downgrade), 0);
        }
        face
    }
}

pub(super) fn fallback_face(primary: &FontFace, cluster: &mut CharCluster) -> Option<Rc<FontFace>> {
    fallback_with_index(primary, cluster, system_font_index)
}

fn fallback_with_index<'a>(
    primary: &FontFace,
    cluster: &mut CharCluster,
    index: impl FnOnce() -> &'a [(String, PathBuf)],
) -> Option<Rc<FontFace>> {
    let primary_font = primary.font_ref();
    let primary_charmap = primary_font.charmap();
    if cluster.map(|ch| primary_charmap.map(ch)) == Status::Complete
        || cluster.info().is_whitespace()
    {
        return None;
    }
    let key = (cluster.chars().len() == 1)
        .then(|| (primary.id(), registry_generation(), cluster.chars()[0].ch));
    if let Some(cached) = key.and_then(|key| CACHE.with(|cache| cache.borrow_mut().character(&key)))
    {
        if let Some(face) = &cached {
            let font = face.font_ref();
            let charmap = font.charmap();
            cluster.map(|ch| charmap.map(ch));
        }
        return cached;
    }
    let mut best = None;
    let complete = REGISTRY.with(|registry| {
        for face in registry.borrow().faces.values() {
            if face.id() == primary.id() {
                continue;
            }
            let font = face.font_ref();
            let charmap = font.charmap();
            match cluster.map(|ch| charmap.map(ch)) {
                Status::Complete => return Some(face.clone()),
                Status::Keep => best = Some(face.clone()),
                Status::Discard => {}
            }
        }
        None
    });
    if let Some(face) = complete {
        return CACHE.with(|cache| cache.borrow_mut().remember(key, Some(face), None));
    }
    let mut selected_path = None;
    for (_, path) in index() {
        let face = CACHE.with(|cache| {
            cache
                .borrow_mut()
                .candidate(path, cluster, FontFace::from_path)
        });
        let Some(face) = face else {
            continue;
        };
        if face.id() == primary.id() {
            continue;
        }
        let font = face.font_ref();
        let charmap = font.charmap();
        match cluster.map(|ch| charmap.map(ch)) {
            Status::Complete => {
                return CACHE.with(|cache| cache.borrow_mut().remember(key, Some(face), Some(path)))
            }
            Status::Keep => {
                best = Some(face);
                selected_path = Some(path.as_path());
            }
            Status::Discard => {}
        }
    }
    CACHE.with(|cache| cache.borrow_mut().remember(key, best, selected_path))
}

#[cfg(test)]
mod tests;
