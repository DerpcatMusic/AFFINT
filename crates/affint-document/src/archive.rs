use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    AssetId, Content, LiveOperation, Mask, NodeId, PageId, PageSize, Parent, RasterAsset, Transform,
};

/// Untrusted versioned artwork, suitable for storage's serializer of choice.
///
/// Deserializing this record does not publish a document. [`crate::Session::from_archive`]
/// is the sole import seam and validates duplicates, allocator watermark, budgets,
/// ownership, finite composed transforms, and all graph dependencies. Input byte
/// limits must also be applied by the storage decoder **before** deserializing;
/// this in-memory crate cannot bound allocations made by an arbitrary deserializer.
/// Unknown fields and enum variants are refused instead of discarded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    /// Initial schema is exactly `1`.
    pub schema: u32,
    /// First never-issued ID, including identities issued on abandoned branches
    /// and canceled gestures. Must exceed every stored page, asset, and layer ID.
    pub next_id: u64,
    /// Pages in document order.
    pub pages: Vec<PageRecord>,
    /// Original sources, each identity appearing exactly once.
    pub assets: Vec<AssetRecord>,
    /// Layers, with explicit parent and ordered child relationships.
    pub layers: Vec<LayerRecord>,
}

/// A serializable bounded page and its ordered roots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageRecord {
    /// Stable page identity.
    pub id: PageId,
    /// Editable display name.
    pub name: String,
    /// Page dimensions.
    pub size: PageSize,
    /// Root layers in back-to-front order.
    pub roots: Vec<NodeId>,
}

/// An immutable full-resolution source record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetRecord {
    /// Stable source identity.
    pub id: AssetId,
    /// Pixels are shared when obtaining or cloning an in-memory archive.
    pub asset: Arc<RasterAsset>,
}

/// Serializable editable layer properties and relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerRecord {
    /// Stable layer identity.
    pub id: NodeId,
    /// Editable display name.
    pub name: String,
    /// Unique ordered artwork owner.
    pub parent: Parent,
    /// Back-to-front child order, empty for leaf layers.
    pub children: Vec<NodeId>,
    /// Original editable content.
    pub content: Content,
    /// Placement relative to owner.
    pub transform: Transform,
    /// Isolated contribution opacity.
    pub opacity: f64,
    /// Normal tree-painting visibility only.
    pub visible: bool,
    /// Optional local coverage dependency.
    pub mask: Option<Mask>,
    /// Ordered non-destructive operations.
    pub operations: Vec<LiveOperation>,
}

pub(crate) fn preflight(archive: &Archive, limits: &crate::Limits) -> Result<(), crate::Error> {
    use crate::validate::check_limit;

    let mut elements = 0usize;
    let mut operations = 0usize;
    let mut edges = 0usize;
    for page in &archive.pages {
        check_limit("name bytes", page.name.len(), limits.max_name_bytes)?;
        edges = edges
            .checked_add(page.roots.len())
            .ok_or(crate::Error::Overflow("tree edges"))?;
    }
    for layer in &archive.layers {
        check_limit("name bytes", layer.name.len(), limits.max_name_bytes)?;
        edges = edges
            .checked_add(layer.children.len())
            .ok_or(crate::Error::Overflow("tree edges"))?;
        operations = operations
            .checked_add(layer.operations.len())
            .ok_or(crate::Error::Overflow("operations"))?;
        if let Content::Vector(vector) = &layer.content {
            elements = elements
                .checked_add(vector.path().elements().len())
                .ok_or(crate::Error::Overflow("path elements"))?;
        }
    }
    check_limit("tree edges", edges, limits.max_layers)?;
    check_limit("operations", operations, limits.max_operations)?;
    check_limit("path elements", elements, limits.max_path_elements)?;
    let mut owners = std::collections::BTreeSet::new();
    let mut pixels = 0usize;
    for record in &archive.assets {
        let owner = record.asset.pixel_owner();
        if owners.insert(Arc::as_ptr(owner) as usize) {
            pixels = pixels
                .checked_add(owner.capacity())
                .ok_or(crate::Error::Overflow("source pixel bytes"))?;
        }
    }
    check_limit("retained pixel bytes", pixels, limits.max_pixel_bytes)
}
