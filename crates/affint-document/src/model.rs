use std::sync::Arc;

use kurbo::{Affine, BezPath, PathEl, Rect};
use serde::{Deserialize, Serialize};

use crate::Error;

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub(crate) u64);

        impl $name {
            /// Stable nonzero value within this document's identity space.
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

identity!(
    NodeId,
    "A stable layer identity; never recycled within a document lineage."
);
identity!(
    AssetId,
    "A stable original-asset identity; never recycled within a document lineage."
);
identity!(
    PageId,
    "A stable page identity; never recycled within a document lineage."
);

/// A process-local snapshot token, unique across sessions and provisional states.
///
/// Undo and redo issue fresh tokens even when artwork matches an older snapshot.
/// Tokens are not serialized; reopening a document creates a new token namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Revision {
    pub(crate) session: u64,
    pub(crate) event: u64,
}

/// Nonzero source pixel dimensions with a checked tightly packed RGBA byte count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "[u32; 2]", into = "[u32; 2]")]
pub struct PixelSize {
    width: u32,
    height: u32,
}

impl PixelSize {
    /// Rejects zero dimensions and byte counts that do not fit the current platform.
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidValue("pixel dimensions"));
        }
        let size = Self { width, height };
        size.byte_len()?;
        Ok(size)
    }

    /// Full source width, never a preview width.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Full source height, never a preview height.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Exact length of tightly packed four-channel pixels.
    pub fn byte_len(self) -> Result<usize, Error> {
        let width = usize::try_from(self.width).map_err(|_| Error::Overflow("pixel width"))?;
        let height = usize::try_from(self.height).map_err(|_| Error::Overflow("pixel height"))?;
        width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(Error::Overflow("pixel bytes"))
    }
}

impl TryFrom<[u32; 2]> for PixelSize {
    type Error = Error;

    fn try_from(value: [u32; 2]) -> Result<Self, Self::Error> {
        Self::new(value[0], value[1])
    }
}

impl From<PixelSize> for [u32; 2] {
    fn from(value: PixelSize) -> Self {
        [value.width, value.height]
    }
}

/// Positive finite page dimensions in document units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 2]", into = "[f64; 2]")]
pub struct PageSize {
    width: f64,
    height: f64,
}

impl PageSize {
    /// Constructs finite positive dimensions.
    pub fn new(width: f64, height: f64) -> Result<Self, Error> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(Error::InvalidValue("page dimensions"));
        }
        Ok(Self { width, height })
    }

    /// Page width in document units.
    pub const fn width(self) -> f64 {
        self.width
    }

    /// Page height in document units.
    pub const fn height(self) -> f64 {
        self.height
    }
}

impl TryFrom<[f64; 2]> for PageSize {
    type Error = Error;

    fn try_from(value: [f64; 2]) -> Result<Self, Self::Error> {
        Self::new(value[0], value[1])
    }
}

impl From<PageSize> for [f64; 2] {
    fn from(value: PageSize) -> Self {
        [value.width, value.height]
    }
}

/// A finite invertible affine placement backed by Kurbo geometry.
///
/// The inverse must also be finite, so raster evaluators can safely map output
/// coordinates to source coordinates. Validation also checks composed tree transforms.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 6]", into = "[f64; 6]")]
pub struct Transform(Affine);

impl Transform {
    /// Identity placement.
    pub const IDENTITY: Self = Self(Affine::IDENTITY);

    /// Constructs a validated placement from Kurbo's `[a, b, c, d, e, f]` coefficients.
    pub fn new(coefficients: [f64; 6]) -> Result<Self, Error> {
        let affine = Affine::new(coefficients);
        if !coefficients.iter().all(|value| value.is_finite())
            || !affine.determinant().is_finite()
            || affine.determinant() == 0.0
            || !affine
                .inverse()
                .as_coeffs()
                .iter()
                .all(|value| value.is_finite())
        {
            return Err(Error::InvalidValue("affine transform"));
        }
        Ok(Self(affine))
    }

    /// The established geometry representation, cheaply copied.
    pub const fn affine(self) -> Affine {
        self.0
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl TryFrom<[f64; 6]> for Transform {
    type Error = Error;

    fn try_from(value: [f64; 6]) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Transform> for [f64; 6] {
    fn from(value: Transform) -> Self {
        value.0.as_coeffs()
    }
}

/// Straight-alpha linear-sRGB color, each component in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 4]", into = "[f64; 4]")]
pub struct Color([f64; 4]);

impl Color {
    /// Constructs an explicitly linear-light fill color.
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Result<Self, Error> {
        let components = [r, g, b, a];
        if !components
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(Error::InvalidValue("linear RGBA color"));
        }
        Ok(Self(components))
    }

    /// Linear red, green, blue, then straight alpha.
    pub const fn components(self) -> [f64; 4] {
        self.0
    }
}

impl TryFrom<[f64; 4]> for Color {
    type Error = Error;

    fn try_from(value: [f64; 4]) -> Result<Self, Self::Error> {
        Self::new(value[0], value[1], value[2], value[3])
    }
}

impl From<Color> for [f64; 4] {
    fn from(value: Color) -> Self {
        value.0
    }
}

/// Original full-resolution tightly packed straight RGBA8 pixels in sRGB.
///
/// No mutable pixel accessor exists. `Clone` only shares the owning allocation,
/// and construction moves the supplied `Vec` without copying its pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RasterInput", into = "RasterInput")]
pub struct RasterAsset {
    size: PixelSize,
    pixels: Arc<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RasterInput {
    size: PixelSize,
    pixels: Arc<Vec<u8>>,
}

impl RasterAsset {
    /// Moves a complete original pixel buffer into immutable shared storage.
    pub fn new(size: PixelSize, pixels: Vec<u8>) -> Result<Self, Error> {
        Self::from_shared(size, Arc::new(pixels))
    }

    fn from_shared(size: PixelSize, pixels: Arc<Vec<u8>>) -> Result<Self, Error> {
        let expected = size.byte_len()?;
        if pixels.len() != expected {
            return Err(Error::PixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self { size, pixels })
    }

    /// Declared full source dimensions.
    pub const fn size(&self) -> PixelSize {
        self.size
    }

    /// Original pixels; all edits and history retain this same allocation.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Bytes retained by the shared original pixel allocation, including spare capacity.
    ///
    /// Consumers that retain this asset beyond a document borrow must account for
    /// this capacity rather than its logical pixel length. Shared clones retain the
    /// same allocation; this method neither copies pixels nor grants mutable access.
    pub fn allocation_bytes(&self) -> usize {
        self.pixels.capacity()
    }

    pub(crate) fn pixel_owner(&self) -> &Arc<Vec<u8>> {
        &self.pixels
    }
}

impl TryFrom<RasterInput> for RasterAsset {
    type Error = Error;

    fn try_from(value: RasterInput) -> Result<Self, Self::Error> {
        Self::from_shared(value.size, value.pixels)
    }
}

impl From<RasterAsset> for RasterInput {
    fn from(value: RasterAsset) -> Self {
        Self {
            size: value.size,
            pixels: value.pixels,
        }
    }
}

/// An immutable filled path backed by Kurbo, using nonzero winding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "VectorInput", into = "VectorInput")]
pub struct VectorContent {
    path: Arc<BezPath>,
    fill: Color,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorInput {
    #[serde(with = "crate::geometry_serde::path")]
    path: Arc<BezPath>,
    fill: Color,
}

impl VectorContent {
    /// Owns finite geometry in shared storage. Every drawing subpath must begin
    /// with `MoveTo`; fill implicitly closes open subpaths as in SVG nonzero fill.
    pub fn new(path: BezPath, fill: Color) -> Result<Self, Error> {
        Self::from_path(&path, fill)
    }

    fn from_path(path: &BezPath, fill: Color) -> Result<Self, Error> {
        if !path.is_finite() {
            return Err(Error::InvalidValue("vector path coordinates"));
        }
        let mut started = false;
        for element in path.elements() {
            match element {
                PathEl::MoveTo(_) => started = true,
                PathEl::ClosePath if started => started = false,
                PathEl::ClosePath => return Err(Error::InvalidValue("vector subpath")),
                _ if !started => return Err(Error::InvalidValue("vector subpath")),
                _ => {}
            }
        }
        // Validate the borrowed geometry before calling Kurbo's constructor, whose
        // first-MoveTo assertion is a programmer precondition. Canonical allocation
        // prevents retaining arbitrary caller spare capacity and copies geometry
        // once; subsequent edits/history share it. Original pixels are unaffected.
        let path = Arc::new(BezPath::from_vec(path.elements().to_vec()));
        Ok(Self { path, fill })
    }

    /// Original editable Bézier geometry, never tessellated or flattened here.
    pub fn path(&self) -> &BezPath {
        &self.path
    }

    /// Nonzero-winding fill color in linear sRGB.
    pub const fn fill(&self) -> Color {
        self.fill
    }

    /// Changes the validated fill while sharing the original immutable geometry.
    ///
    /// The already validated path is neither copied nor canonicalized again.
    /// Earlier content and history keep their own fill and the same path owner.
    pub fn with_fill(&self, fill: Color) -> Self {
        Self {
            path: Arc::clone(&self.path),
            fill,
        }
    }
}

impl TryFrom<VectorInput> for VectorContent {
    type Error = Error;

    fn try_from(value: VectorInput) -> Result<Self, Self::Error> {
        Self::from_path(&value.path, value.fill)
    }
}

impl From<VectorContent> for VectorInput {
    fn from(value: VectorContent) -> Self {
        Self {
            path: value.path,
            fill: value.fill,
        }
    }
}

/// Editable source content. Only groups may have children.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Content {
    /// Isolated source-over composition of the ordered children.
    Group,
    /// Original raster pixels, located at `(0, 0)` with one local unit per pixel.
    Raster {
        /// Immutable original source.
        asset: AssetId,
    },
    /// Filled editable Bézier geometry in layer-local coordinates.
    Vector(VectorContent),
}

/// Where a layer appears in the ordered artwork tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Parent {
    /// Top-level artwork of a page.
    Page(PageId),
    /// Child artwork of a group.
    Layer(NodeId),
}

/// Editable coverage sampling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskMode {
    /// Straight alpha of the evaluated mask source.
    Alpha,
    /// Rec. 709 linear luminance multiplied by source alpha.
    Luminance,
}

/// A coverage dependency evaluated in the masked layer's local coordinates.
///
/// The source's own affine, operations, mask, and opacity apply; its normal tree
/// visibility and ancestor transforms do not. Hiding a mask source suppresses
/// ordinary artwork painting but does not disable this dependency. `None` disables
/// a layer's mask. A source can simultaneously appear as ordinary visible artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    /// Referenced source layer.
    pub source: NodeId,
    /// Alpha or linear-luminance coverage.
    pub mode: MaskMode,
    /// Complements the resulting coverage.
    pub inverted: bool,
}

/// Live operations evaluated in stored order without changing original content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiveOperation {
    /// Keeps coverage inside this finite positive-area local rectangle. Source
    /// coordinates and pixels outside it remain intact and editable.
    Crop(#[serde(with = "crate::geometry_serde::rect")] Rect),
    /// Multiplies linear RGB by `2^stops`, preserving alpha. Stops are finite in
    /// `[-32, 32]`; processing may retain values above one until output conversion.
    Exposure(f64),
}

/// A layer in a validated immutable snapshot.
///
/// Composition order is source -> live operations -> mask -> opacity -> affine ->
/// parent source-over composition. Child lists run back-to-front. Group opacity
/// applies to the isolated group result, not individually to each child.
#[derive(Debug, Clone)]
pub struct Layer {
    pub(crate) name: Arc<str>,
    pub(crate) parent: Parent,
    pub(crate) children: Arc<Vec<NodeId>>,
    pub(crate) content: Content,
    pub(crate) transform: Transform,
    pub(crate) opacity: f64,
    pub(crate) visible: bool,
    pub(crate) mask: Option<Mask>,
    pub(crate) operations: Arc<Vec<LiveOperation>>,
}

impl Layer {
    /// Editable display name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Ordered owner in the artwork tree.
    pub const fn parent(&self) -> Parent {
        self.parent
    }
    /// Back-to-front child order; empty for non-group layers.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
    /// Original editable content.
    pub const fn content(&self) -> &Content {
        &self.content
    }
    /// Local placement relative to the parent.
    pub const fn transform(&self) -> Transform {
        self.transform
    }
    /// Isolated contribution opacity in `[0, 1]`.
    pub const fn opacity(&self) -> f64 {
        self.opacity
    }
    /// Whether normal tree painting includes the layer; mask dependencies ignore this.
    pub const fn visible(&self) -> bool {
        self.visible
    }
    /// Editable coverage dependency, independent of tree visibility.
    pub const fn mask(&self) -> Option<Mask> {
        self.mask
    }
    /// Ordered editable local operations.
    pub fn operations(&self) -> &[LiveOperation] {
        &self.operations
    }
}

/// A bounded page and its ordered top-level layers.
#[derive(Debug, Clone)]
pub struct Page {
    pub(crate) name: Arc<str>,
    pub(crate) size: PageSize,
    pub(crate) roots: Arc<Vec<NodeId>>,
}

impl Page {
    /// Editable display name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Page dimensions in document units.
    pub const fn size(&self) -> PageSize {
        self.size
    }
    /// Top-level artwork, ordered back-to-front.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }
}
