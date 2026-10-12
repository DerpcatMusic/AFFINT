//! Immutable artwork and validated, atomic, reversible edits.
//!
//! [`Session`] is the mutation seam. Its snapshots share persistent map structure,
//! geometry, and full-resolution source pixels. A failed edit never changes artwork
//! or history. Identity allocation is deliberately irreversible, including canceled
//! gestures. [`Archive`] is an untrusted interchange record; importing it validates
//! resources, all references, tree ownership, and render dependencies before publication.
//!
//! Supported artwork is groups, filled Bézier paths, and straight RGBA8 sRGB raster
//! sources, with affine placement, local crops, linear-light exposure, and layer masks.
//! Text, strokes, blend modes other than source-over, vector booleans, raster painting,
//! color profiles, external formats, and unknown operations are not represented yet.
//! Importers must refuse unsupported content rather than flatten or discard it.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod archive;
mod edit;
mod geometry_serde;
mod model;
mod session;
mod staging;
mod validate;

pub use archive::{Archive, AssetRecord, LayerRecord, PageRecord};
pub use edit::{Change, Created, Edit, History, LayerSpec};
pub use model::{
    AssetId, Color, Content, Layer, LiveOperation, Mask, MaskMode, NodeId, Page, PageId, PageSize,
    Parent, PixelSize, RasterAsset, Revision, Transform, VectorContent,
};
pub use session::{HistoryStatus, Limits, ResourceUsage, Session, Snapshot};

/// A rejected edit or import. No variant indicates partially published artwork.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// A scalar or geometry contains NaN, infinity, or an invalid range.
    #[error("invalid {0}")]
    InvalidValue(&'static str),
    /// Pixel storage does not match the exact declared dimensions.
    #[error("pixel length is {actual}, expected {expected}")]
    PixelLength {
        /// Required tightly packed RGBA byte count.
        expected: usize,
        /// Supplied byte count.
        actual: usize,
    },
    /// An input-derived length or counter overflowed.
    #[error("{0} overflow")]
    Overflow(&'static str),
    /// An identity is zero, duplicated, or is above the saved allocation watermark.
    #[error("invalid or duplicated identity {0}")]
    InvalidIdentity(u64),
    /// A layer reference does not exist in the staged artwork.
    #[error("missing layer {0:?}")]
    MissingLayer(NodeId),
    /// An asset reference does not exist in the staged artwork.
    #[error("missing asset {0:?}")]
    MissingAsset(AssetId),
    /// A page reference does not exist in the staged artwork.
    #[error("missing page {0:?}")]
    MissingPage(PageId),
    /// Only groups can own child layers.
    #[error("layer {0:?} is not a group")]
    NotAGroup(NodeId),
    /// A child list and its layer's recorded parent disagree.
    #[error("inconsistent parent relationship for layer {0:?}")]
    ParentMismatch(NodeId),
    /// Parentage and/or mask dependencies contain a cycle.
    #[error("cyclic layer dependencies")]
    Cycle,
    /// A child/page insertion index is beyond the available slots.
    #[error("index {index} exceeds length {len}")]
    InvalidIndex {
        /// Requested index.
        index: usize,
        /// Available insertion length.
        len: usize,
    },
    /// Deletion would leave a mask reference pointing at a missing layer.
    #[error("layer {target:?} is still used by mask on {user:?}")]
    LayerInUse {
        /// Layer the edit attempted to remove.
        target: NodeId,
        /// Surviving layer that still requires it.
        user: NodeId,
    },
    /// A configured resource budget was exceeded.
    #[error("{resource} uses {actual}, limit {limit}")]
    LimitExceeded {
        /// Named resource.
        resource: &'static str,
        /// Staged amount.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// A gesture is already active; ordinary edits and history are excluded.
    #[error("a gesture is active")]
    GestureActive,
    /// A gesture operation requires an active gesture.
    #[error("no gesture is active")]
    NoGesture,
    /// The requested undo/redo side is empty.
    #[error("no {0} history")]
    EmptyHistory(&'static str),
    /// An edit has no changes or no label.
    #[error("an edit needs a nonempty label and changes")]
    EmptyEdit,
    /// The serialized schema is not understood. Unsupported content is refused.
    #[error("unsupported document schema {0}")]
    UnsupportedSchema(u32),
}
