use std::sync::Arc;

use crate::{
    AssetId, Content, LiveOperation, Mask, NodeId, PageId, PageSize, Parent, RasterAsset, Transform,
};

/// An atomic, named batch of meaningful artwork changes.
///
/// Changes are staged in order, then the complete result is validated once. The
/// whole batch succeeds or fails. IDs created by a batch are returned only on
/// success; an inserted raster can create its source and layer in the same batch.
/// Submission consumes this batch. Live-operation buffers are moved into staging
/// rather than cloned; spare capacity may be reduced when the resulting allocation
/// fits the metadata policy. Final artwork and retained-state validation remain
/// authoritative, including when a later change replaces an earlier payload.
#[derive(Debug, Clone)]
pub struct Edit {
    pub(crate) label: String,
    pub(crate) changes: Vec<Change>,
}

impl Edit {
    /// Creates a named batch. Empty labels or batches are rejected by the session.
    pub fn new(label: impl Into<String>, changes: impl IntoIterator<Item = Change>) -> Self {
        Self {
            label: label.into(),
            changes: changes.into_iter().collect(),
        }
    }
}

/// Initial properties of a newly inserted layer.
///
/// Fields may be edited before submission; session validation is authoritative.
/// Use [`LayerSpec::new`] for visible, fully opaque identity placement defaults.
#[derive(Debug, Clone)]
pub struct LayerSpec {
    /// Editable display name.
    pub name: String,
    /// Shared editable source content.
    pub content: Content,
    /// Placement relative to its owner.
    pub transform: Transform,
    /// Isolated contribution opacity in `[0, 1]`.
    pub opacity: f64,
    /// Inclusion in ordinary painting, independent of mask dependencies.
    pub visible: bool,
    /// Optional layer-local mask dependency.
    pub mask: Option<Mask>,
    /// Ordered live operations.
    pub operations: Vec<LiveOperation>,
}

impl LayerSpec {
    /// Creates a visible, fully opaque, unmasked layer with identity placement.
    pub fn new(name: impl Into<String>, content: Content) -> Self {
        Self {
            name: name.into(),
            content,
            transform: Transform::IDENTITY,
            opacity: 1.0,
            visible: true,
            mask: None,
            operations: Vec::new(),
        }
    }
}

/// Concrete supported changes; unsupported editor features have no pretend commands.
#[derive(Debug, Clone)]
pub enum Change {
    /// Appends a bounded page. Creation returns its stable identity.
    AddPage {
        /// Display name.
        name: String,
        /// Positive finite dimensions.
        size: PageSize,
    },
    /// Changes a page's name and dimensions without altering its artwork.
    SetPage {
        /// Existing page.
        page: PageId,
        /// New display name.
        name: String,
        /// New dimensions.
        size: PageSize,
    },
    /// Adds an immutable original source independently of a layer.
    AddAsset {
        /// Original full-resolution source. Cloning this handle shares pixels.
        asset: Arc<RasterAsset>,
    },
    /// Inserts a new layer at an explicit back-to-front index.
    InsertLayer {
        /// Existing page or group owner.
        parent: Parent,
        /// Insertion slot in the owner's current list, including the end.
        index: usize,
        /// Initial editable properties.
        layer: LayerSpec,
    },
    /// Atomically inserts a source asset and a raster layer referring to it.
    InsertRaster {
        /// Existing page or group owner.
        parent: Parent,
        /// Insertion slot in the owner's current list.
        index: usize,
        /// Display name.
        name: String,
        /// Immutable original full-resolution source.
        asset: Arc<RasterAsset>,
        /// Initial placement.
        transform: Transform,
    },
    /// Changes editable layer properties while preserving its content and identity.
    SetLayer {
        /// Existing layer.
        node: NodeId,
        /// New editable properties. Group content can only remain a group if it
        /// has children; validation rejects replacing it with leaf content.
        layer: LayerSpec,
    },
    /// Replaces only the local affine, useful for continuous placement gestures.
    SetTransform {
        /// Existing layer.
        node: NodeId,
        /// Validated local placement.
        transform: Transform,
    },
    /// Replaces only an editable mask dependency.
    SetMask {
        /// Existing masked layer.
        node: NodeId,
        /// New dependency; `None` disables the mask.
        mask: Option<Mask>,
    },
    /// Replaces only the ordered live operations.
    SetOperations {
        /// Existing layer.
        node: NodeId,
        /// Replacement editable operations.
        operations: Vec<LiveOperation>,
    },
    /// Moves a layer, preserving local coordinates. To preserve world placement,
    /// submit a corresponding `SetTransform` in the same batch.
    MoveLayer {
        /// Existing layer, including its subtree.
        node: NodeId,
        /// Destination page or group.
        parent: Parent,
        /// Index in the destination list **after** removing the moving layer.
        /// This definition also applies to a reorder within the same owner.
        index: usize,
    },
    /// Deletes a layer and its subtree. Sources remain in the document asset table;
    /// surviving masks must be cleared/repointed in the same batch.
    RemoveLayer {
        /// Existing subtree root.
        node: NodeId,
    },
}

/// Stable identities created by a successfully published batch, in command order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Created {
    /// New page identities.
    pub pages: Vec<PageId>,
    /// New immutable asset identities.
    pub assets: Vec<AssetId>,
    /// New layer identities.
    pub nodes: Vec<NodeId>,
}

/// Which reversible history direction to traverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum History {
    /// Restore the artwork preceding the most recent committed edit.
    Undo,
    /// Restore the artwork following the most recently undone edit.
    Redo,
}
