//! Immutable exact-work retention. Hashes and revision tokens are not semantic keys.
//!
//! Entries pin only reached source owners and bounded semantic records, never a
//! Snapshot. Cache and renderer leases follow the actual immutable allocation,
//! including an entry borrowed after eviction. Callbacks run outside the map lock.

use std::mem::size_of;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use affint_document::{
    Content, LiveOperation, Mask, NodeId, RasterAsset, Snapshot, Transform, VectorContent,
};
use kurbo::{Affine, PathEl, Point};

use super::spatial::VectorBounds;
use super::{
    COVERAGE_SCALE, COVERAGE_SIDE, Pixel, Tile, check_cancel, has_curves, periodic_cancel,
};
use crate::{
    Error, RetainedStats,
    budget::{Budget, Buffer, Lease},
};

const ARC_HEADER: usize = 2 * size_of::<usize>();

#[derive(Debug)]
struct Charge {
    // Owning data fields precede these charges in every retained allocation.
    renderer: Lease,
    _cache: Option<Lease>,
}

#[derive(Debug, Clone)]
enum Entry {
    Tile(Arc<RetainedTile>),
    Geometry(Arc<Geometry>),
    Source(Arc<Source>),
}

impl Entry {
    fn unborrowed(&self) -> bool {
        match self {
            Self::Tile(value) => Arc::strong_count(value) == 1,
            Self::Geometry(value) => Arc::strong_count(value) == 1,
            Self::Source(value) => Arc::strong_count(value) == 1,
        }
    }
    fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Tile(a), Self::Tile(b)) => Arc::ptr_eq(a, b),
            (Self::Geometry(a), Self::Geometry(b)) => Arc::ptr_eq(a, b),
            (Self::Source(a), Self::Source(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
struct Slot {
    stamp: u64,
    entry: Entry,
}

#[derive(Debug)]
struct Slots {
    data: Buffer<Option<Slot>>,
    _cache: Option<Lease>,
}

#[derive(Debug)]
struct Sources {
    data: Buffer<Option<Weak<Source>>>,
    // Weak handles keep Arc control storage alive after Source's value drops.
    // Charge one conservative control/value slot for every registry slot.
    _charge: Charge,
}

#[derive(Debug)]
struct OwnerBytes {
    bytes: usize,
    total: Arc<AtomicUsize>,
}

impl OwnerBytes {
    fn new(bytes: usize, total: &Arc<AtomicUsize>) -> Self {
        total.fetch_add(bytes, Ordering::AcqRel);
        Self {
            bytes,
            total: Arc::clone(total),
        }
    }
}

impl Drop for OwnerBytes {
    fn drop(&mut self) {
        self.total.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Debug, Default)]
struct State {
    slots: Option<Slots>,
    stamp: u64,
}

#[derive(Debug, Default)]
struct Counters {
    tile_hits: AtomicU64,
    tile_misses: AtomicU64,
    evaluated_tiles: AtomicU64,
    geometry_hits: AtomicU64,
    geometry_misses: AtomicU64,
    prepared_curves: AtomicU64,
    preflight_reclaims: AtomicU64,
    evictions: AtomicU64,
    refused: AtomicU64,
}

fn increment(counter: &AtomicU64) {
    let _previous = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}

#[derive(Debug)]
pub(crate) struct RetainedCache {
    renderer: Arc<Budget>,
    resident: Arc<Budget>,
    slot_count: usize,
    max_bytes: usize,
    state: Mutex<State>,
    sources: Mutex<Option<Sources>>,
    source_bytes: Arc<AtomicUsize>,
    pixel_bytes: Arc<AtomicUsize>,
    counters: Counters,
}

impl RetainedCache {
    pub(crate) fn new(renderer: Arc<Budget>, max_bytes: usize) -> Self {
        // Empty/rejected layer tiles and geometry entries are much smaller than
        // full color tiles. Byte admission remains authoritative for both.
        let tile_bytes = COVERAGE_SIDE as usize * COVERAGE_SIDE as usize;
        Self {
            renderer,
            resident: Arc::new(Budget::new(max_bytes)),
            slot_count: (max_bytes / tile_bytes).clamp(256, 4096),
            max_bytes,
            state: Mutex::new(State::default()),
            sources: Mutex::new(None),
            source_bytes: Arc::new(AtomicUsize::new(0)),
            pixel_bytes: Arc::new(AtomicUsize::new(0)),
            counters: Counters::default(),
        }
    }

    pub(crate) fn stats(&self) -> RetainedStats {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        RetainedStats {
            tile_hits: read(&self.counters.tile_hits),
            tile_misses: read(&self.counters.tile_misses),
            evaluated_tiles: read(&self.counters.evaluated_tiles),
            geometry_hits: read(&self.counters.geometry_hits),
            geometry_misses: read(&self.counters.geometry_misses),
            prepared_curves: read(&self.counters.prepared_curves),
            preflight_reclaims: read(&self.counters.preflight_reclaims),
            evictions: read(&self.counters.evictions),
            refused: read(&self.counters.refused),
            resident_bytes: self.resident.used(),
            source_bytes: self.source_bytes.load(Ordering::Acquire),
            retained_pixel_bytes: self.pixel_bytes.load(Ordering::Acquire),
        }
    }

    pub(crate) fn evaluated(&self) {
        increment(&self.counters.evaluated_tiles);
    }

    pub(super) fn reclaimed_preflight(&self) {
        increment(&self.counters.preflight_reclaims);
    }

    pub(crate) fn clear(&self) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(slots) = state.slots.take() {
            for _slot in slots.data.data.iter().flatten() {
                increment(&self.counters.evictions);
            }
            drop(slots);
        }
        drop(state);
        self.prune_sources();
    }

    fn prune_sources(&self) {
        let mut sources = match self.sources.lock() {
            Ok(sources) => sources,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(registry) = &mut *sources {
            for source in &mut registry.data.data {
                if source
                    .as_ref()
                    .is_some_and(|source| source.strong_count() == 0)
                {
                    *source = None;
                }
            }
            if registry.data.data.iter().all(Option::is_none) {
                *sources = None;
            }
        }
    }

    fn evict_one(&self) -> bool {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let Some(slots) = &mut state.slots else {
            return false;
        };
        let oldest = slots
            .data
            .data
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.as_ref()
                    .filter(|slot| slot.entry.unborrowed())
                    .map(|slot| (index, slot.stamp))
            })
            .min_by_key(|(_, stamp)| *stamp);
        if let Some((index, _)) = oldest {
            slots.data.data[index] = None;
            increment(&self.counters.evictions);
            true
        } else {
            false
        }
    }

    // Optional retention cannot erase an evaluation error. Only memory/admission
    // refusal becomes an uncached path; arithmetic/cancellation errors propagate.
    fn admit(&self, bytes: usize) -> Result<Option<Charge>, Error> {
        loop {
            let cache = match self.resident.reserve(bytes) {
                Ok(lease) => lease,
                Err(Error::BudgetExceeded { .. }) => {
                    if self.evict_one() {
                        continue;
                    }
                    increment(&self.counters.refused);
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            match self.renderer.reserve(bytes) {
                Ok(renderer) => {
                    return Ok(Some(Charge {
                        renderer,
                        _cache: Some(cache),
                    }));
                }
                Err(Error::BudgetExceeded { .. }) => {
                    drop(cache);
                    if self.evict_one() {
                        continue;
                    }
                    increment(&self.counters.refused);
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn candidate(&self, start: usize, matches: impl Fn(&Entry) -> bool) -> Option<(usize, Entry)> {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state
            .slots
            .as_ref()?
            .data
            .data
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, slot)| {
                slot.as_ref()
                    .filter(|slot| matches(&slot.entry))
                    .map(|slot| (index, slot.entry.clone()))
            })
    }

    fn touch(&self, index: usize, candidate: &Entry) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.stamp = state.stamp.saturating_add(1);
        let stamp = state.stamp;
        if let Some(slot) = state
            .slots
            .as_mut()
            .and_then(|slots| slots.data.data.get_mut(index))
            .and_then(Option::as_mut)
        {
            if slot.entry.same_owner(candidate) {
                slot.stamp = stamp;
            }
        }
    }

    fn insert(&self, entry: Entry) -> Result<(), Error> {
        // Allocate bounded map storage outside its lock, so admission may evict.
        let needs_slots = match self.state.lock() {
            Ok(state) => state.slots.is_none(),
            Err(poisoned) => poisoned.into_inner().slots.is_none(),
        };
        let candidate_slots = if needs_slots {
            let bytes = self
                .slot_count
                .checked_mul(size_of::<Option<Slot>>())
                .ok_or(Error::Overflow("retained map bytes"))?;
            let Some(mut charge) = self.admit(bytes)? else {
                return Ok(());
            };
            let buffer = match Buffer::with_lease(
                self.slot_count,
                None,
                "retained map slots",
                charge.renderer.split(bytes)?,
            ) {
                Ok(buffer) => buffer,
                Err(Error::AllocationFailed { .. } | Error::BudgetExceeded { .. }) => {
                    increment(&self.counters.refused);
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            // Buffer capacities are admitted by Buffer. Charge the same actual
            // capacity to the resident ceiling rather than just the requested len.
            let actual = buffer
                .data
                .capacity()
                .checked_mul(size_of::<Option<Slot>>())
                .ok_or(Error::Overflow("retained map capacity"))?;
            if actual != bytes {
                increment(&self.counters.refused);
                return Ok(());
            }
            Some(Slots {
                data: buffer,
                _cache: charge._cache,
            })
        } else {
            None
        };
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state.slots.is_none() {
            state.slots = candidate_slots;
        }
        state.stamp = state.stamp.saturating_add(1);
        let stamp = state.stamp;
        let Some(slots) = &mut state.slots else {
            return Ok(());
        };
        let index = slots
            .data
            .data
            .iter()
            .position(Option::is_none)
            .or_else(|| {
                slots
                    .data
                    .data
                    .iter()
                    .enumerate()
                    .filter_map(|(index, slot)| slot.as_ref().map(|slot| (index, slot.stamp)))
                    .min_by_key(|(_, stamp)| *stamp)
                    .map(|(index, _)| index)
            });
        if let Some(index) = index {
            if slots.data.data[index].is_some() {
                increment(&self.counters.evictions);
            }
            slots.data.data[index] = Some(Slot { stamp, entry });
        }
        Ok(())
    }

    pub(super) fn tile(
        &self,
        snapshot: &Snapshot,
        key: TileKey,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<RetainedTile>>, Error> {
        let TileKey {
            id,
            parent,
            mask_source,
            tile,
            width,
            height,
        } = key;
        let mut start = 0;
        while let Some((index, candidate)) = self.candidate(start, |entry| match entry {
            Entry::Tile(value) => {
                value.key.id == id
                    && value.key.mask_source == mask_source
                    && value.key.width == width
                    && value.key.height == height
                    && value.key.tile.width == tile.width
                    && value.key.tile.height == tile.height
            }
            _ => false,
        }) {
            check_cancel(canceled)?;
            start = index + 1;
            let Entry::Tile(value) = &candidate else {
                continue;
            };
            if value.recipe.same_mapping(
                parent,
                tile,
                value.key.parent,
                value.key.tile,
                canceled,
            )? && value.recipe.matches(snapshot, id, mask_source, canceled)?
            {
                check_cancel(canceled)?;
                self.touch(index, &candidate);
                increment(&self.counters.tile_hits);
                return Ok(Some(Arc::clone(value)));
            }
        }
        increment(&self.counters.tile_misses);
        Ok(None)
    }

    pub(super) fn retain_tile(
        &self,
        snapshot: &Snapshot,
        key: TileKey,
        pixels: Option<Buffer<Pixel>>,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Option<TilePixels>, Error> {
        check_cancel(canceled)?;
        let Some(recipe) = self.recipe(snapshot, key.id, key.mask_source, canceled)? else {
            return Ok(pixels.map(TilePixels::Scratch));
        };
        let bytes = pixels
            .as_ref()
            .map_or(0, |pixels| pixels.data.capacity())
            .checked_mul(size_of::<Pixel>())
            .ok_or(Error::Overflow("retained tile capacity"))?;
        let charge_bytes = bytes
            .checked_add(size_of::<RetainedTile>() + ARC_HEADER)
            .ok_or(Error::Overflow("retained tile owner"))?;
        let Some(mut charge) = self.admit(charge_bytes)? else {
            return Ok(pixels.map(TilePixels::Scratch));
        };
        // Transfer the allocation, never clone its pixels or let a local scratch
        // lease escape the enclosing job's reservation.
        let pixels = match pixels {
            Some(Buffer { data, lease }) => {
                let pixels = Buffer {
                    data,
                    lease: charge.renderer.split(bytes)?,
                };
                drop(lease);
                Some(pixels)
            }
            None => None,
        };
        let contributes = pixels.is_some();
        let value = Arc::new(RetainedTile {
            pixels,
            recipe,
            key,
            _pixels: OwnerBytes::new(bytes, &self.pixel_bytes),
            _charge: charge,
        });
        check_cancel(canceled)?;
        self.insert(Entry::Tile(Arc::clone(&value)))?;
        Ok(contributes.then_some(TilePixels::Retained(value)))
    }

    fn recipe(
        &self,
        snapshot: &Snapshot,
        id: NodeId,
        mask_source: bool,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<Recipe>>, Error> {
        let mut start = 0;
        while let Some((index, Entry::Tile(value))) = self.candidate(start, |entry| matches!(entry, Entry::Tile(value) if value.key.id == id && value.key.mask_source == mask_source)) {
            check_cancel(canceled)?;
            start = index + 1;
            if value.key.id == id && value.key.mask_source == mask_source && value.recipe.matches(snapshot, id, mask_source, canceled)? {
                return Ok(Some(Arc::clone(&value.recipe)));
            }
        }
        let mut count = 0usize;
        let mut bytes = size_of::<Recipe>() + ARC_HEADER;
        let fits = walk(
            snapshot,
            id,
            mask_source,
            1,
            canceled,
            &mut |_, layer, _asset| {
                count = count
                    .checked_add(1)
                    .ok_or(Error::Overflow("retained recipe records"))?;
                add(&mut bytes, 2 * size_of::<Record>())?;
                add(
                    &mut bytes,
                    layer
                        .children()
                        .len()
                        .checked_mul(2 * size_of::<NodeId>())
                        .ok_or(Error::Overflow("retained children"))?,
                )?;
                add(
                    &mut bytes,
                    layer
                        .operations()
                        .len()
                        .checked_mul(2 * size_of::<LiveOperation>())
                        .ok_or(Error::Overflow("retained operations"))?,
                )?;
                if let Content::Vector(vector) = layer.content() {
                    add(&mut bytes, vector_owner_bytes(vector)?)?;
                }
                Ok(bytes <= self.max_bytes)
            },
        )?;
        if !fits {
            increment(&self.counters.refused);
            return Ok(None);
        }
        let Some(charge) = self.admit(bytes)? else {
            return Ok(None);
        };
        let mut records = Vec::new();
        if records.try_reserve_exact(count).is_err() {
            increment(&self.counters.refused);
            return Ok(None);
        }
        let built = walk(
            snapshot,
            id,
            mask_source,
            1,
            canceled,
            &mut |id, layer, asset| {
                let source = match asset {
                    Some(asset) => self.source(asset, canceled)?,
                    None => None,
                };
                if asset.is_some() && source.is_none() {
                    return Ok(false);
                }
                records.push(Record {
                    id,
                    content: layer.content().clone(),
                    transform: layer.transform(),
                    opacity: layer.opacity(),
                    visible: layer.visible(),
                    mask: layer.mask(),
                    children: copy_slice(layer.children(), "retained child order")?,
                    operations: copy_slice(layer.operations(), "retained operations")?,
                    asset: source,
                });
                Ok(true)
            },
        );
        match built {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(Error::AllocationFailed { .. }) => {
                increment(&self.counters.refused);
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
        let mut actual = size_of::<Recipe>() + ARC_HEADER;
        add(
            &mut actual,
            records
                .capacity()
                .checked_mul(size_of::<Record>())
                .ok_or(Error::Overflow("retained record capacity"))?,
        )?;
        for record in &records {
            add(
                &mut actual,
                record
                    .children
                    .capacity()
                    .checked_mul(size_of::<NodeId>())
                    .ok_or(Error::Overflow("retained child capacity"))?,
            )?;
            add(
                &mut actual,
                record
                    .operations
                    .capacity()
                    .checked_mul(size_of::<LiveOperation>())
                    .ok_or(Error::Overflow("retained operation capacity"))?,
            )?;
            if let Content::Vector(vector) = &record.content {
                add(&mut actual, vector_owner_bytes(vector)?)?;
            }
        }
        if actual > bytes {
            increment(&self.counters.refused);
            return Ok(None);
        }
        Ok(Some(Arc::new(Recipe {
            records,
            _charge: charge,
        })))
    }

    pub(super) fn geometry(
        &self,
        id: NodeId,
        vector: &VectorContent,
        placement: Affine,
        width: u32,
        height: u32,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<Geometry>>, Error> {
        let mut start = 0;
        while let Some((index, candidate)) = self.candidate(start, |entry| matches!(entry, Entry::Geometry(value) if value.id == id && value.width == width && value.height == height && affine_bits(value.placement) == affine_bits(placement))) {
            check_cancel(canceled)?;
            start = index + 1;
            let Entry::Geometry(value) = &candidate else { continue; };
            if value.id == id && value.width == width && value.height == height && affine_bits(value.placement) == affine_bits(placement) && same_path(&value.vector, vector, canceled)? {
                check_cancel(canceled)?;
                self.touch(index, &candidate);
                increment(&self.counters.geometry_hits);
                return Ok(Some(Arc::clone(value)));
            }
        }
        increment(&self.counters.geometry_misses);
        Ok(None)
    }

    pub(super) fn retain_geometry(
        &self,
        key: GeometryKey<'_>,
        count: usize,
        support: VectorBounds,
        canceled: &dyn Fn() -> bool,
        populate: impl FnOnce(&mut [PathEl]) -> Result<(), Error>,
    ) -> Result<Option<Arc<Geometry>>, Error> {
        let bytes = count
            .checked_mul(size_of::<PathEl>())
            .ok_or(Error::Overflow("retained derived geometry"))?;
        let charge_bytes = bytes
            .checked_add(vector_owner_bytes(key.vector)?)
            .and_then(|bytes| bytes.checked_add(size_of::<Geometry>() + ARC_HEADER))
            .ok_or(Error::Overflow("retained geometry owner"))?;
        let mut charge = match self.admit(charge_bytes)? {
            Some(charge) => charge,
            None => match self.renderer.reserve(charge_bytes) {
                Ok(renderer) => Charge {
                    renderer,
                    _cache: None,
                },
                Err(Error::BudgetExceeded { .. }) => return Ok(None),
                Err(error) => return Err(error),
            },
        };
        let mut path = match Buffer::with_lease(
            count,
            PathEl::ClosePath,
            "retained device geometry",
            charge.renderer.split(bytes)?,
        ) {
            Ok(path) => path,
            Err(Error::AllocationFailed { .. } | Error::BudgetExceeded { .. }) => {
                increment(&self.counters.refused);
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if path.data.capacity() != count {
            increment(&self.counters.refused);
            return Ok(None);
        }
        populate(&mut path.data)?;
        increment(&self.counters.prepared_curves);
        check_cancel(canceled)?;
        let retained = charge._cache.is_some();
        let value = Arc::new(Geometry {
            path,
            vector: key.vector.clone(),
            id: key.id,
            placement: key.placement,
            width: key.width,
            height: key.height,
            support,
            _pixels: retained.then(|| OwnerBytes::new(bytes, &self.pixel_bytes)),
            _charge: charge,
        });
        if retained {
            self.insert(Entry::Geometry(Arc::clone(&value)))?;
        }
        Ok(Some(value))
    }

    fn source(
        &self,
        asset: &RasterAsset,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Option<Arc<Source>>, Error> {
        check_cancel(canceled)?;
        // Serialize source interning independently of the entry map. Weak slots
        // discover active evicted owners too, preventing duplicate source charges
        // under concurrent misses or multiple recipes using the same allocation.
        let mut sources = match self.sources.lock() {
            Ok(sources) => sources,
            Err(poisoned) => poisoned.into_inner(),
        };
        let same = |source: &Source| {
            source.asset.size() == asset.size()
                && source.asset.pixels().as_ptr() == asset.pixels().as_ptr()
        };
        if let Some(registry) = &mut *sources {
            for slot in &mut registry.data.data {
                match slot.as_ref().and_then(Weak::upgrade) {
                    Some(source) if same(&source) => return Ok(Some(source)),
                    Some(_) => {}
                    None => *slot = None,
                }
            }
        }
        if sources.is_none() {
            let vector_bytes = self
                .slot_count
                .checked_mul(size_of::<Option<Weak<Source>>>())
                .ok_or(Error::Overflow("retained source registry"))?;
            let bytes = self
                .slot_count
                .checked_mul(size_of::<Source>() + ARC_HEADER)
                .and_then(|bytes| bytes.checked_add(vector_bytes))
                .ok_or(Error::Overflow("retained source controls"))?;
            let Some(mut charge) = self.admit(bytes)? else {
                return Ok(None);
            };
            let data = match Buffer::with_lease(
                self.slot_count,
                None,
                "retained source registry",
                charge.renderer.split(vector_bytes)?,
            ) {
                Ok(data) => data,
                Err(Error::AllocationFailed { .. } | Error::BudgetExceeded { .. }) => {
                    increment(&self.counters.refused);
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            if data.data.capacity() != self.slot_count {
                increment(&self.counters.refused);
                return Ok(None);
            }
            *sources = Some(Sources {
                data,
                _charge: charge,
            });
        }
        let Some(registry) = &mut *sources else {
            return Ok(None);
        };
        let Some(slot) = registry.data.data.iter_mut().find(|slot| slot.is_none()) else {
            increment(&self.counters.refused);
            return Ok(None);
        };
        let bytes = asset
            .allocation_bytes()
            .checked_add(size_of::<Vec<u8>>() + ARC_HEADER)
            .ok_or(Error::Overflow("retained original owner"))?;
        let Some(charge) = self.admit(bytes)? else {
            return Ok(None);
        };
        let source = Arc::new(Source {
            asset: asset.clone(),
            _pixels: OwnerBytes::new(asset.allocation_bytes(), &self.source_bytes),
            _charge: charge,
        });
        *slot = Some(Arc::downgrade(&source));
        drop(sources);
        check_cancel(canceled)?;
        self.insert(Entry::Source(Arc::clone(&source)))?;
        Ok(Some(source))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TileKey {
    pub(super) id: NodeId,
    pub(super) parent: Affine,
    pub(super) mask_source: bool,
    pub(super) tile: Tile,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) struct GeometryKey<'a> {
    pub(super) id: NodeId,
    pub(super) vector: &'a VectorContent,
    pub(super) placement: Affine,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Debug)]
pub(super) struct Geometry {
    pub(super) path: Buffer<PathEl>,
    vector: VectorContent,
    pub(super) id: NodeId,
    pub(super) placement: Affine,
    width: u32,
    height: u32,
    pub(super) support: VectorBounds,
    _pixels: Option<OwnerBytes>,
    _charge: Charge,
}

#[derive(Debug)]
pub(super) struct RetainedTile {
    pub(super) pixels: Option<Buffer<Pixel>>,
    recipe: Arc<Recipe>,
    key: TileKey,
    _pixels: OwnerBytes,
    _charge: Charge,
}

pub(super) enum TilePixels {
    Scratch(Buffer<Pixel>),
    Retained(Arc<RetainedTile>),
}

impl TilePixels {
    pub(super) fn data(&self) -> &[Pixel] {
        match self {
            Self::Scratch(buffer) => &buffer.data,
            Self::Retained(value) => value
                .pixels
                .as_ref()
                .map_or(&[], |pixels| pixels.data.as_slice()),
        }
    }
}

#[derive(Debug)]
struct Record {
    id: NodeId,
    content: Content,
    transform: Transform,
    opacity: f64,
    visible: bool,
    mask: Option<Mask>,
    children: Vec<NodeId>,
    operations: Vec<LiveOperation>,
    asset: Option<Arc<Source>>,
}

#[derive(Debug)]
struct Source {
    asset: RasterAsset,
    _pixels: OwnerBytes,
    _charge: Charge,
}

#[derive(Debug)]
struct Recipe {
    records: Vec<Record>,
    _charge: Charge,
}

impl Recipe {
    fn matches(
        &self,
        snapshot: &Snapshot,
        id: NodeId,
        mask_source: bool,
        canceled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
        let mut records = self.records.iter();
        let equal = walk(
            snapshot,
            id,
            mask_source,
            1,
            canceled,
            &mut |id, layer, asset| {
                let Some(record) = records.next() else {
                    return Ok(false);
                };
                let mut equal = record.id == id
                    && affine_bits(record.transform.affine())
                        == affine_bits(layer.transform().affine())
                    && record.opacity.to_bits() == layer.opacity().to_bits()
                    && record.visible == layer.visible()
                    && record.mask == layer.mask()
                    && record.children == layer.children()
                    && same_operations(&record.operations, layer.operations(), canceled)?
                    && same_content(&record.content, layer.content(), canceled)?;
                equal &= match (&record.asset, asset) {
                    (Some(a), Some(b)) => {
                        a.asset.size() == b.size()
                            && a.asset.pixels().len() == b.pixels().len()
                            && a.asset.pixels().as_ptr() == b.pixels().as_ptr()
                    }
                    (None, None) => true,
                    _ => false,
                };
                Ok(equal)
            },
        )?;
        Ok(equal && records.next().is_none())
    }

    fn same_mapping(
        &self,
        new_parent: Affine,
        new_tile: Tile,
        old_parent: Affine,
        old_tile: Tile,
        canceled: &dyn Fn() -> bool,
    ) -> Result<bool, Error> {
        if affine_bits(new_parent) == affine_bits(old_parent) && new_tile == old_tile {
            return Ok(true);
        }
        // A shift to another canonical block retains the precise clipping grid.
        // Partial blocks and arbitrary scratch tiling deliberately miss.
        let canonical = |tile: Tile| {
            tile.x % COVERAGE_SIDE == 0
                && tile.y % COVERAGE_SIDE == 0
                && tile.width == COVERAGE_SIDE
                && tile.height == COVERAGE_SIDE
        };
        if !canonical(new_tile) || !canonical(old_tile) {
            return Ok(false);
        }
        let mut records = self.records.iter();
        same_record_mapping(
            &mut records,
            new_parent,
            new_tile,
            old_parent,
            old_tile,
            true,
            canceled,
        )
    }
}

fn same_record_mapping(
    records: &mut std::slice::Iter<'_, Record>,
    new_parent: Affine,
    new_tile: Tile,
    old_parent: Affine,
    old_tile: Tile,
    mask_source: bool,
    canceled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check_cancel(canceled)?;
    let Some(record) = records.next() else {
        return Ok(false);
    };
    if !mask_source && !record.visible {
        return Ok(true);
    }
    let new = new_parent * record.transform.affine();
    let old = old_parent * record.transform.affine();
    let line_mapping = || {
        affine_bits(coverage_mapping(new, new_tile)) == affine_bits(coverage_mapping(old, old_tile))
    };
    let mut equal = match &record.content {
        Content::Group => true,
        Content::Vector(vector) => !has_curves(vector.path().elements()) && line_mapping(),
        Content::Raster { .. } => {
            match (sample_mapping(new, new_tile), sample_mapping(old, old_tile)) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            }
        }
    };
    if record
        .operations
        .iter()
        .any(|operation| matches!(operation, LiveOperation::Crop(_)))
    {
        equal &= line_mapping();
    }
    if !equal {
        return Ok(false);
    }
    if matches!(record.content, Content::Group) {
        for _ in &record.children {
            if !same_record_mapping(records, new, new_tile, old, old_tile, false, canceled)? {
                return Ok(false);
            }
        }
    }
    if record.mask.is_some() {
        return same_record_mapping(records, new, new_tile, old, old_tile, true, canceled);
    }
    Ok(true)
}

fn coverage_mapping(placement: Affine, tile: Tile) -> Affine {
    Affine::scale(f64::from(COVERAGE_SCALE))
        * Affine::translate((-f64::from(tile.x), -f64::from(tile.y)))
        * placement
}

// A sufficient exact-binary domain, not an approximate camera bucket. Diagonal
// powers of two in +/-16 and a 2^-17 translation grid produce at most 49 useful
// bits below 2^31. Every source sample multiply/add is exactly representable in
// f64. Equal linear coefficients and the first sample prove every pixel's actual
// inverse * absolute sample expression identical. Other affines simply miss.
fn sample_mapping(placement: Affine, tile: Tile) -> Option<[u64; 4]> {
    let inverse = placement.inverse();
    let [a, b, c, d, e, f] = inverse.as_coeffs();
    if b != 0.0 || c != 0.0 {
        return None;
    }
    let power = |value: f64| (-16..=16).any(|exponent| value.abs() == 2.0f64.powi(exponent));
    let grid = |value: f64| {
        value.is_finite() && value.abs() <= 1_073_741_824.0 && (value * 131_072.0).fract() == 0.0
    };
    if !power(a) || !power(d) || !grid(e) || !grid(f) {
        return None;
    }
    for (coefficient, edge) in [(a, tile.x + tile.width), (d, tile.y + tile.height)] {
        if (coefficient * (f64::from(edge) + 0.5)).abs() > 1_073_741_824.0 {
            return None;
        }
    }
    let origin = inverse * Point::new(f64::from(tile.x) + 0.5, f64::from(tile.y) + 0.5);
    Some([
        a.to_bits(),
        d.to_bits(),
        origin.x.to_bits(),
        origin.y.to_bits(),
    ])
}

fn walk(
    snapshot: &Snapshot,
    id: NodeId,
    mask_source: bool,
    depth: usize,
    canceled: &dyn Fn() -> bool,
    visit: &mut impl FnMut(NodeId, &affint_document::Layer, Option<&RasterAsset>) -> Result<bool, Error>,
) -> Result<bool, Error> {
    check_cancel(canceled)?;
    if depth > 128 {
        return Err(Error::DepthExceeded {
            node: id,
            actual: depth,
            limit: 128,
        });
    }
    let layer = snapshot.node(id).ok_or(Error::MissingLayer(id))?;
    let asset = match layer.content() {
        Content::Raster { asset } => {
            Some(snapshot.asset(*asset).ok_or(Error::MissingAsset(*asset))?)
        }
        _ => None,
    };
    if !visit(id, layer, asset)? {
        return Ok(false);
    }
    if !mask_source && !layer.visible() {
        return Ok(true);
    }
    if matches!(layer.content(), Content::Group) {
        for &child in layer.children() {
            if !walk(snapshot, child, false, depth + 1, canceled, visit)? {
                return Ok(false);
            }
        }
    }
    if let Some(mask) = layer.mask() {
        if !walk(snapshot, mask.source, true, depth + 1, canceled, visit)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn affine_bits(affine: Affine) -> [u64; 6] {
    affine.as_coeffs().map(f64::to_bits)
}

fn same_content(a: &Content, b: &Content, canceled: &dyn Fn() -> bool) -> Result<bool, Error> {
    Ok(match (a, b) {
        (Content::Group, Content::Group) => true,
        (Content::Raster { asset: a }, Content::Raster { asset: b }) => a == b,
        (Content::Vector(a), Content::Vector(b)) => {
            a.fill().components().map(f64::to_bits) == b.fill().components().map(f64::to_bits)
                && same_path(a, b, canceled)?
        }
        _ => false,
    })
}

fn same_path(
    a: &VectorContent,
    b: &VectorContent,
    canceled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    if std::ptr::eq(a.path(), b.path()) {
        return Ok(true);
    }
    let a = a.path().elements();
    let b = b.path().elements();
    if a.len() != b.len() {
        return Ok(false);
    }
    for (index, (a, b)) in a.iter().zip(b).enumerate() {
        periodic_cancel(canceled, index)?;
        let point_equal =
            |a: Point, b: Point| a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits();
        let equal = match (*a, *b) {
            (PathEl::MoveTo(a), PathEl::MoveTo(b)) | (PathEl::LineTo(a), PathEl::LineTo(b)) => {
                point_equal(a, b)
            }
            (PathEl::QuadTo(a1, a2), PathEl::QuadTo(b1, b2)) => {
                point_equal(a1, b1) && point_equal(a2, b2)
            }
            (PathEl::CurveTo(a1, a2, a3), PathEl::CurveTo(b1, b2, b3)) => {
                point_equal(a1, b1) && point_equal(a2, b2) && point_equal(a3, b3)
            }
            (PathEl::ClosePath, PathEl::ClosePath) => true,
            _ => false,
        };
        if !equal {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_operations(
    a: &[LiveOperation],
    b: &[LiveOperation],
    canceled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (index, (a, b)) in a.iter().zip(b).enumerate() {
        periodic_cancel(canceled, index)?;
        let equal = match (a, b) {
            (LiveOperation::Crop(a), LiveOperation::Crop(b)) => {
                [a.x0, a.y0, a.x1, a.y1].map(f64::to_bits)
                    == [b.x0, b.y0, b.x1, b.y1].map(f64::to_bits)
            }
            (LiveOperation::Exposure(a), LiveOperation::Exposure(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        };
        if !equal {
            return Ok(false);
        }
    }
    Ok(true)
}

fn vector_owner_bytes(vector: &VectorContent) -> Result<usize, Error> {
    // Document construction canonicalizes the path rather than retaining caller
    // capacity. Match its conservative fourfold metadata admission policy.
    vector
        .path()
        .elements()
        .len()
        .checked_mul(4 * size_of::<PathEl>())
        .and_then(|bytes| bytes.checked_add(size_of::<kurbo::BezPath>() + ARC_HEADER))
        .ok_or(Error::Overflow("retained original geometry"))
}

fn add(total: &mut usize, bytes: usize) -> Result<(), Error> {
    *total = total
        .checked_add(bytes)
        .ok_or(Error::Overflow("retained recipe bytes"))?;
    Ok(())
}

fn copy_slice<T: Clone>(source: &[T], resource: &'static str) -> Result<Vec<T>, Error> {
    let bytes = source
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(Error::Overflow(resource))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| Error::AllocationFailed { resource, bytes })?;
    output.extend_from_slice(source);
    Ok(output)
}
