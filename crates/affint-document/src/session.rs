use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU64, Ordering},
};

use rpds::RedBlackTreeMapSync;

use crate::validate::{check_limit, validate};
use crate::{
    Archive, AssetId, AssetRecord, Created, Edit, Error, History, Layer, LayerRecord, NodeId, Page,
    PageId, PageRecord, PageSize, RasterAsset, Revision,
};

static SESSION_IDS: AtomicU64 = AtomicU64::new(1);

/// Explicit per-session resource budgets. Counts bound validation work; bytes bound
/// owned source allocations and conservatively charged metadata/history/staging.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum pages in one artwork state.
    pub max_pages: usize,
    /// Maximum layers in one artwork state.
    pub max_layers: usize,
    /// Maximum original sources in one artwork state.
    pub max_assets: usize,
    /// Maximum combined live operations in one artwork state.
    pub max_operations: usize,
    /// Maximum combined Bézier path elements in one artwork state.
    pub max_path_elements: usize,
    /// Maximum bytes in one display/edit label.
    pub max_name_bytes: usize,
    /// Maximum tree or render-dependency depth; iterative validation avoids recursion.
    pub max_depth: usize,
    /// Maximum unique owned pixel allocation bytes across current, history,
    /// staging, and outstanding snapshots. Allocated capacity, not just length,
    /// is charged. A source allocation shared under multiple asset IDs counts once.
    pub max_pixel_bytes: usize,
    /// Maximum conservative metadata charge across history, current artwork, and
    /// one staged state. Unchanged shared structures are deliberately overcharged;
    /// this is a bound for admission, not measured resident memory.
    pub max_metadata_bytes: usize,
    /// Maximum number of reversible edits retained across undo and redo.
    pub max_history_entries: usize,
    /// Maximum conservative metadata charge retained in undo/redo. Oldest undo
    /// entries, then farthest redo entries, are evicted after history transitions
    /// until both history budgets are satisfied.
    pub max_history_metadata_bytes: usize,
    /// Maximum changes in one edit or accepted updates of one entire gesture,
    /// bounding staging work and the gesture's accumulated creation records.
    pub max_changes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_pages: 1_024,
            max_layers: 100_000,
            max_assets: 10_000,
            max_operations: 100_000,
            max_path_elements: 2_000_000,
            max_name_bytes: 16_384,
            max_depth: 256,
            max_pixel_bytes: 512 * 1024 * 1024,
            max_metadata_bytes: 256 * 1024 * 1024,
            max_history_entries: 100,
            max_history_metadata_bytes: 128 * 1024 * 1024,
            max_changes: 100_000,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct State {
    pub(crate) pages: RedBlackTreeMapSync<PageId, Page>,
    pub(crate) page_order: Arc<Vec<PageId>>,
    pub(crate) assets: RedBlackTreeMapSync<AssetId, Arc<RasterAsset>>,
    pub(crate) layers: RedBlackTreeMapSync<NodeId, Layer>,
    pub(crate) metadata_bytes: usize,
}

impl State {
    pub(crate) fn empty() -> Self {
        Self {
            pages: RedBlackTreeMapSync::new_sync(),
            page_order: Arc::new(Vec::new()),
            assets: RedBlackTreeMapSync::new_sync(),
            layers: RedBlackTreeMapSync::new_sync(),
            metadata_bytes: 0,
        }
    }
}

/// A cheap thread-safe immutable artwork handle for rendering, saving, and inspection.
///
/// Content never changes. The identity allocation watermark used by [`Self::archive`]
/// may increase as its originating session allocates identities, preserving non-reuse
/// when an old snapshot is saved after undo, branching, or gesture cancellation.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub(crate) state: Arc<State>,
    revision: Revision,
    watermark: Arc<AtomicU64>,
}

impl Snapshot {
    /// Exact token for worker results; provisional states have distinct tokens.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Pages in stable document order, distinct from identity-sorted `pages()`.
    pub fn page_order(&self) -> &[PageId] {
        &self.state.page_order
    }
    /// Inspects pages in identity order without exposing mutable structure.
    pub fn pages(&self) -> impl Iterator<Item = (&PageId, &Page)> {
        self.state.pages.iter()
    }
    /// Inspects layers in identity order.
    pub fn layers(&self) -> impl Iterator<Item = (&NodeId, &Layer)> {
        self.state.layers.iter()
    }
    /// Inspects immutable source assets in identity order.
    pub fn assets(&self) -> impl Iterator<Item = (&AssetId, &RasterAsset)> {
        self.state
            .assets
            .iter()
            .map(|(id, asset)| (id, asset.as_ref()))
    }
    /// Looks up a bounded page by stable identity.
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.state.pages.get(&id)
    }
    /// Looks up a layer by stable identity.
    pub fn node(&self, id: NodeId) -> Option<&Layer> {
        self.state.layers.get(&id)
    }
    /// Borrows original pixels without copying their owning allocation.
    pub fn asset(&self, id: AssetId) -> Option<&RasterAsset> {
        self.state.assets.get(&id).map(Arc::as_ref)
    }

    /// Obtains an unflattened serializable record; original assets and path geometry
    /// remain shared in memory. Serialization itself belongs to the storage module.
    pub fn archive(&self) -> Result<Archive, Error> {
        Ok(Archive {
            schema: 1,
            next_id: self.watermark.load(Ordering::Relaxed),
            pages: self
                .page_order()
                .iter()
                .map(|id| {
                    let page = self.page(*id).ok_or(Error::MissingPage(*id))?;
                    Ok(PageRecord {
                        id: *id,
                        name: page.name().to_owned(),
                        size: page.size(),
                        roots: page.roots().to_vec(),
                    })
                })
                .collect::<Result<_, Error>>()?,
            assets: self
                .state
                .assets
                .iter()
                .map(|(id, asset)| AssetRecord {
                    id: *id,
                    asset: Arc::clone(asset),
                })
                .collect(),
            layers: self
                .layers()
                .map(|(id, layer)| LayerRecord {
                    id: *id,
                    name: layer.name().to_owned(),
                    parent: layer.parent(),
                    children: layer.children().to_vec(),
                    content: layer.content().clone(),
                    transform: layer.transform(),
                    opacity: layer.opacity(),
                    visible: layer.visible(),
                    mask: layer.mask(),
                    operations: layer.operations().to_vec(),
                })
                .collect(),
        })
    }
}

#[derive(Debug)]
struct Entry {
    state: Arc<State>,
    label: Arc<str>,
}

#[derive(Debug)]
struct Gesture {
    label: Arc<str>,
    preview: Snapshot,
    created: Created,
    changed: bool,
    changes: usize,
}

struct StagedEdit {
    state: Arc<State>,
    created: Created,
    label: String,
}

/// Observable undo/redo availability and next labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryStatus<'a> {
    /// Number of retained undo entries.
    pub undo: usize,
    /// Number of retained redo entries.
    pub redo: usize,
    /// Most recent undoable edit's name.
    pub undo_label: Option<&'a str>,
    /// Most recent redoable edit's name.
    pub redo_label: Option<&'a str>,
}

/// Current admitted source bytes and conservative metadata charges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceUsage {
    /// Unique live pixel allocation capacities, including outstanding snapshots.
    pub pixel_bytes: usize,
    /// Current artwork's conservative metadata charge.
    pub current_metadata_bytes: usize,
    /// Retained undo/redo states' conservative metadata charge.
    pub history_metadata_bytes: usize,
    /// Unique live states' conservative charge, including outstanding snapshots
    /// and active previews. Shared internals can be charged more than once.
    pub retained_metadata_bytes: usize,
}

/// The document's single mutation owner. Consumers receive immutable snapshots.
///
/// ID allocation lies outside reversible history and is shared with snapshots;
/// failed edits may burn identities but never alter artwork or history. Only one
/// gesture is active at a time. Undo, redo, and ordinary edits are excluded during
/// a gesture, whose previews are explicit and whose committed snapshot stays intact.
#[derive(Debug)]
pub struct Session {
    current: Snapshot,
    limits: Limits,
    event: u64,
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    gesture: Option<Gesture>,
    pixel_owners: Vec<Weak<Vec<u8>>>,
    state_owners: Vec<Weak<State>>,
}

impl Session {
    /// Creates one empty page named `Page 1`, with no initial undo history.
    pub fn new(size: PageSize, limits: Limits) -> Result<Self, Error> {
        let mut state = State::empty();
        let id = PageId(1);
        state.pages.insert_mut(
            id,
            Page {
                name: Arc::from("Page 1"),
                size,
                roots: Arc::new(Vec::new()),
            },
        );
        state.page_order = Arc::new(vec![id]);
        state.metadata_bytes = validate(&state, &limits, 2)?;
        Self::from_state(state, 2, limits)
    }

    /// Validates untrusted artwork and a never-reuse allocation watermark before
    /// creating a session. History is intentionally not a serialized file feature.
    pub fn from_archive(archive: Archive, limits: Limits) -> Result<Self, Error> {
        if archive.schema != 1 {
            return Err(Error::UnsupportedSchema(archive.schema));
        }
        check_limit("pages", archive.pages.len(), limits.max_pages)?;
        check_limit("layers", archive.layers.len(), limits.max_layers)?;
        check_limit("assets", archive.assets.len(), limits.max_assets)?;
        crate::archive::preflight(&archive, &limits)?;
        let next_id = archive.next_id;
        let mut state = State::empty();
        let mut identities = std::collections::BTreeSet::new();
        let mut order = Vec::with_capacity(archive.pages.len());
        for page in archive.pages {
            admit_identity(&mut identities, page.id.get(), next_id)?;
            order.push(page.id);
            state.pages.insert_mut(
                page.id,
                Page {
                    name: Arc::from(page.name),
                    size: page.size,
                    roots: Arc::new(page.roots),
                },
            );
        }
        state.page_order = Arc::new(order);
        for asset in archive.assets {
            admit_identity(&mut identities, asset.id.get(), next_id)?;
            state.assets.insert_mut(asset.id, asset.asset);
        }
        for layer in archive.layers {
            admit_identity(&mut identities, layer.id.get(), next_id)?;
            state.layers.insert_mut(
                layer.id,
                Layer {
                    name: Arc::from(layer.name),
                    parent: layer.parent,
                    children: Arc::new(layer.children),
                    content: layer.content,
                    transform: layer.transform,
                    opacity: layer.opacity,
                    visible: layer.visible,
                    mask: layer.mask,
                    operations: Arc::new(layer.operations),
                },
            );
        }
        state.metadata_bytes = validate(&state, &limits, next_id)?;
        Self::from_state(state, next_id, limits)
    }

    fn from_state(state: State, next_id: u64, limits: Limits) -> Result<Self, Error> {
        if next_id == 0 {
            return Err(Error::InvalidIdentity(next_id));
        }
        check_limit(
            "metadata bytes",
            state.metadata_bytes,
            limits.max_metadata_bytes,
        )?;
        let session = allocate(&SESSION_IDS, "session identities")?;
        let current = Snapshot {
            state: Arc::new(state),
            revision: Revision { session, event: 0 },
            watermark: Arc::new(AtomicU64::new(next_id)),
        };
        let mut value = Self {
            current,
            limits,
            event: 0,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
            pixel_owners: Vec::new(),
            state_owners: Vec::new(),
        };
        value.check_pixels(&value.current.state)?;
        let state = Arc::clone(&value.current.state);
        value.register_pixels(&state);
        value.register_state(&state);
        check_limit(
            "retained metadata bytes",
            value.live_metadata_bytes(None)?,
            value.limits.max_metadata_bytes,
        )?;
        Ok(value)
    }

    /// Cheap immutable committed snapshot. Gesture previews are returned explicitly
    /// by `update_gesture` and never replace this committed state before commit.
    pub fn snapshot(&self) -> Snapshot {
        self.current.clone()
    }

    /// Whether a worker result belongs to the current committed artwork.
    pub fn is_current(&self, revision: Revision) -> bool {
        self.current.revision == revision
    }

    /// Applies one atomic named edit and returns newly allocated identities.
    pub fn transact(&mut self, edit: Edit) -> Result<Created, Error> {
        if self.gesture.is_some() {
            return Err(Error::GestureActive);
        }
        let StagedEdit {
            state,
            created,
            label,
        } = self.stage(&self.current.state, edit)?;
        let revision = self.next_revision()?;
        let label: Arc<str> = Arc::from(label);
        self.undo.push_back(Entry {
            state: Arc::clone(&self.current.state),
            label,
        });
        self.redo.clear();
        self.register_pixels(&state);
        self.register_state(&state);
        self.current = self.snapshot_for(state, revision);
        self.trim_history();
        Ok(created)
    }

    /// Traverses one retained edit. Restored artwork gets a fresh revision, while
    /// layer/source identities and original pixel allocations remain unchanged.
    pub fn history(&mut self, direction: History) -> Result<Snapshot, Error> {
        if self.gesture.is_some() {
            return Err(Error::GestureActive);
        }
        let available = match direction {
            History::Undo => !self.undo.is_empty(),
            History::Redo => !self.redo.is_empty(),
        };
        if !available {
            return Err(Error::EmptyHistory(match direction {
                History::Undo => "undo",
                History::Redo => "redo",
            }));
        }
        let revision = self.next_revision()?;
        let entry = match direction {
            History::Undo => self.undo.pop_back().ok_or(Error::EmptyHistory("undo"))?,
            History::Redo => self.redo.pop().ok_or(Error::EmptyHistory("redo"))?,
        };
        let reverse = Entry {
            state: Arc::clone(&self.current.state),
            label: entry.label,
        };
        match direction {
            History::Undo => self.redo.push(reverse),
            History::Redo => self.undo.push_back(reverse),
        }
        self.current = self.snapshot_for(entry.state, revision);
        self.trim_history();
        Ok(self.snapshot())
    }

    /// Inspect bounded history without exposing internal snapshots or mutation.
    pub fn history_status(&self) -> HistoryStatus<'_> {
        HistoryStatus {
            undo: self.undo.len(),
            redo: self.redo.len(),
            undo_label: self.undo.back().map(|entry| entry.label.as_ref()),
            redo_label: self.redo.last().map(|entry| entry.label.as_ref()),
        }
    }

    /// Starts an isolated gesture. Accepted updates are coalesced into one edit.
    pub fn begin_gesture(&mut self, label: impl Into<String>) -> Result<(), Error> {
        if self.gesture.is_some() {
            return Err(Error::GestureActive);
        }
        let label = label.into();
        self.check_label(&label)?;
        let metadata = self
            .live_metadata_bytes(None)?
            .checked_add(label.len())
            .and_then(|bytes| bytes.checked_add(size_of::<Gesture>()))
            .ok_or(Error::Overflow("gesture metadata"))?;
        check_limit(
            "retained metadata bytes",
            metadata,
            self.limits.max_metadata_bytes,
        )?;
        self.gesture = Some(Gesture {
            label: Arc::from(label),
            preview: self.snapshot(),
            created: Created::default(),
            changed: false,
            changes: 0,
        });
        Ok(())
    }

    /// Applies a validated incremental gesture update to the preceding preview.
    /// Failure retains the preceding preview, committed artwork, and history.
    /// Returned revisions are unique and accepted IDs remain reserved on cancel.
    pub fn update_gesture(&mut self, edit: Edit) -> Result<(Snapshot, Created), Error> {
        let gesture = self.gesture.as_ref().ok_or(Error::NoGesture)?;
        let changes = gesture
            .changes
            .checked_add(edit.changes.len())
            .ok_or(Error::Overflow("gesture changes"))?;
        check_limit("gesture changes", changes, self.limits.max_changes)?;
        let base = Arc::clone(&gesture.preview.state);
        let StagedEdit { state, created, .. } = self.stage(&base, edit)?;
        let revision = self.next_revision()?;
        self.register_pixels(&state);
        self.register_state(&state);
        let preview = self.snapshot_for(state, revision);
        let gesture = self.gesture.as_mut().ok_or(Error::NoGesture)?;
        gesture.preview = preview.clone();
        gesture.created.pages.extend_from_slice(&created.pages);
        gesture.created.assets.extend_from_slice(&created.assets);
        gesture.created.nodes.extend_from_slice(&created.nodes);
        gesture.changed = true;
        gesture.changes = changes;
        Ok((preview, created))
    }

    /// Publishes the final gesture preview as exactly one undoable edit. Committing
    /// a gesture without any accepted updates produces no artwork/history change.
    pub fn commit_gesture(&mut self) -> Result<Created, Error> {
        let changed = self.gesture.as_ref().ok_or(Error::NoGesture)?.changed;
        if !changed {
            self.gesture = None;
            return Ok(Created::default());
        }
        let revision = self.next_revision()?;
        let gesture = self.gesture.take().ok_or(Error::NoGesture)?;
        self.undo.push_back(Entry {
            state: Arc::clone(&self.current.state),
            label: gesture.label,
        });
        self.redo.clear();
        self.current = self.snapshot_for(gesture.preview.state, revision);
        self.trim_history();
        Ok(gesture.created)
    }

    /// Discards all previews. Committed artwork and undo/redo remain exactly as
    /// before the gesture, and accepted provisional IDs are never reused.
    pub fn cancel_gesture(&mut self) -> Result<(), Error> {
        if self.gesture.take().is_none() {
            return Err(Error::NoGesture);
        }
        Ok(())
    }

    /// Reports current source ownership and conservative metadata charges.
    pub fn resource_usage(&self) -> Result<ResourceUsage, Error> {
        Ok(ResourceUsage {
            pixel_bytes: self.live_pixel_bytes(None)?,
            current_metadata_bytes: self.current.state.metadata_bytes,
            history_metadata_bytes: self.history_metadata_bytes()?,
            retained_metadata_bytes: self.live_metadata_bytes(None)?,
        })
    }

    fn stage(&self, base: &State, edit: Edit) -> Result<StagedEdit, Error> {
        self.check_label(&edit.label)?;
        if edit.changes.is_empty() {
            return Err(Error::EmptyEdit);
        }
        check_limit("edit changes", edit.changes.len(), self.limits.max_changes)?;
        let change_count = edit.changes.len();
        let Edit { label, changes } = edit;
        let (mut state, created) =
            crate::staging::apply(base, changes, &self.current.watermark, &self.limits)?;
        state.metadata_bytes = validate(
            &state,
            &self.limits,
            self.current.watermark.load(Ordering::Relaxed),
        )?;
        self.check_pixels(&state)?;
        let metadata = self
            .live_metadata_bytes(Some(&state))?
            .checked_add(label.len())
            .and_then(|bytes| bytes.checked_add(2 * size_of::<Entry>()))
            .ok_or(Error::Overflow("edit metadata"))?;
        let metadata = if self.gesture.is_some() {
            let added = change_count
                .checked_mul(4 * size_of::<u64>())
                .ok_or(Error::Overflow("gesture metadata"))?;
            metadata
                .checked_add(added)
                .ok_or(Error::Overflow("gesture metadata"))?
        } else {
            metadata
        };
        check_limit(
            "retained metadata bytes",
            metadata,
            self.limits.max_metadata_bytes,
        )?;
        Ok(StagedEdit {
            state: Arc::new(state),
            created,
            label,
        })
    }

    fn check_label(&self, label: &str) -> Result<(), Error> {
        if label.trim().is_empty() {
            return Err(Error::EmptyEdit);
        }
        check_limit("name bytes", label.len(), self.limits.max_name_bytes)
    }

    fn next_revision(&mut self) -> Result<Revision, Error> {
        self.event = self
            .event
            .checked_add(1)
            .ok_or(Error::Overflow("revisions"))?;
        Ok(Revision {
            session: self.current.revision.session,
            event: self.event,
        })
    }

    fn snapshot_for(&self, state: Arc<State>, revision: Revision) -> Snapshot {
        Snapshot {
            state,
            revision,
            watermark: Arc::clone(&self.current.watermark),
        }
    }

    fn history_metadata_bytes(&self) -> Result<usize, Error> {
        self.undo
            .iter()
            .chain(self.redo.iter())
            .try_fold(0usize, |bytes, entry| {
                bytes
                    .checked_add(entry.state.metadata_bytes)
                    .and_then(|bytes| bytes.checked_add(entry.label.len()))
                    .and_then(|bytes| bytes.checked_add(2 * size_of::<Entry>()))
                    .ok_or(Error::Overflow("history metadata"))
            })
    }

    fn trim_history(&mut self) {
        // Admission uses checked arithmetic. Saturation here conservatively treats
        // any unrepresentable retained charge as above the history budget.
        while self.undo.len().saturating_add(self.redo.len()) > self.limits.max_history_entries
            || self
                .undo
                .iter()
                .chain(self.redo.iter())
                .fold(0usize, |bytes, entry| {
                    bytes
                        .saturating_add(entry.state.metadata_bytes)
                        .saturating_add(entry.label.len())
                        .saturating_add(2 * size_of::<Entry>())
                })
                > self.limits.max_history_metadata_bytes
        {
            if self.undo.pop_front().is_none() {
                if self.redo.is_empty() {
                    break;
                }
                self.redo.remove(0);
            }
        }
    }

    fn check_pixels(&self, staged: &State) -> Result<(), Error> {
        check_limit(
            "retained pixel bytes",
            self.live_pixel_bytes(Some(staged))?,
            self.limits.max_pixel_bytes,
        )
    }

    fn live_pixel_bytes(&self, staged: Option<&State>) -> Result<usize, Error> {
        let mut owners = std::collections::BTreeMap::new();
        for owner in &self.pixel_owners {
            if let Some(owner) = owner.upgrade() {
                owners.insert(Arc::as_ptr(&owner) as usize, owner);
            }
        }
        if let Some(staged) = staged {
            for (_, asset) in staged.assets.iter() {
                let owner = asset.pixel_owner();
                owners
                    .entry(Arc::as_ptr(owner) as usize)
                    .or_insert_with(|| Arc::clone(owner));
            }
        }
        owners.values().try_fold(0usize, |bytes, owner| {
            bytes
                .checked_add(owner.capacity())
                .ok_or(Error::Overflow("retained pixel bytes"))
        })
    }

    fn register_pixels(&mut self, state: &State) {
        self.pixel_owners.retain(|owner| owner.strong_count() > 0);
        let mut known: std::collections::BTreeSet<usize> = self
            .pixel_owners
            .iter()
            .map(|owner| owner.as_ptr() as usize)
            .collect();
        for (_, asset) in state.assets.iter() {
            let pixels = asset.pixel_owner();
            if known.insert(Arc::as_ptr(pixels) as usize) {
                self.pixel_owners.push(Arc::downgrade(pixels));
            }
        }
    }

    fn live_metadata_bytes(&self, staged: Option<&State>) -> Result<usize, Error> {
        let mut states = std::collections::BTreeMap::new();
        for owner in &self.state_owners {
            if let Some(state) = owner.upgrade() {
                states.insert(Arc::as_ptr(&state) as usize, state);
            }
        }
        let mut bytes = states.values().try_fold(0usize, |bytes, state| {
            bytes
                .checked_add(state.metadata_bytes)
                .ok_or(Error::Overflow("retained metadata"))
        })?;
        if let Some(state) = staged {
            bytes = bytes
                .checked_add(state.metadata_bytes)
                .ok_or(Error::Overflow("staged metadata"))?;
        }
        for entry in self.undo.iter().chain(self.redo.iter()) {
            let charge = entry
                .label
                .len()
                .checked_add(2 * size_of::<Entry>())
                .ok_or(Error::Overflow("history labels"))?;
            bytes = bytes
                .checked_add(charge)
                .ok_or(Error::Overflow("history labels"))?;
        }
        if let Some(gesture) = &self.gesture {
            // Each accepted change can create at most an asset and a layer. The
            // factor of four also covers geometric Vec capacity growth.
            let charge = gesture
                .changes
                .checked_mul(4 * size_of::<u64>())
                .and_then(|charge| charge.checked_add(gesture.label.len()))
                .and_then(|charge| charge.checked_add(size_of::<Gesture>()))
                .ok_or(Error::Overflow("gesture metadata"))?;
            bytes = bytes
                .checked_add(charge)
                .ok_or(Error::Overflow("gesture metadata"))?;
        }
        let state_capacity = if staged.is_some() {
            self.state_owners
                .capacity()
                .saturating_add(4)
                .saturating_mul(2)
        } else {
            self.state_owners.capacity()
        };
        let pixel_capacity = if let Some(state) = staged {
            self.pixel_owners
                .capacity()
                .saturating_add(state.assets.size())
                .saturating_add(4)
                .saturating_mul(2)
        } else {
            self.pixel_owners.capacity()
        };
        let registry = state_capacity
            .checked_mul(size_of::<Weak<State>>())
            .and_then(|charge| {
                pixel_capacity
                    .checked_mul(size_of::<Weak<Vec<u8>>>())
                    .and_then(|pixels| charge.checked_add(pixels))
            })
            .ok_or(Error::Overflow("owner registry bytes"))?;
        bytes
            .checked_add(registry)
            .ok_or(Error::Overflow("owner registry bytes"))
    }

    fn register_state(&mut self, state: &Arc<State>) {
        self.state_owners.retain(|owner| owner.strong_count() > 0);
        let weak = Arc::downgrade(state);
        if !self.state_owners.iter().any(|owner| owner.ptr_eq(&weak)) {
            self.state_owners.push(weak);
        }
    }
}

fn admit_identity(
    ids: &mut std::collections::BTreeSet<u64>,
    id: u64,
    next_id: u64,
) -> Result<(), Error> {
    if id == 0 || id >= next_id || !ids.insert(id) {
        return Err(Error::InvalidIdentity(id));
    }
    Ok(())
}

pub(crate) fn allocate(counter: &AtomicU64, name: &'static str) -> Result<u64, Error> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Error::Overflow(name))
}
