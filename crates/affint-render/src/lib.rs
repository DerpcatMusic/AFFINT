//! Exact-resolution CPU reference rendering of committed AFFINT artwork.
//!
//! The sole color policy is f32 premultiplied linear sRGB throughout evaluation,
//! followed by explicitly requested straight RGBA8 sRGB export or unclipped
//! linear-premultiplied RGBA f32 working output. Original raster pixels are sampled
//! directly with bilinear interpolation and a transparent border. Filled Bézier
//! paths and crops use tiny-skia's antialiased, 8-bit nonzero-winding coverage
//! on a fixed output-anchored 256-pixel grid independent of color scratch tiling.
//! Render-only f64 Kurbo flattening targets 1/1024 output pixel before line scanning;
//! editable originals retain their Bézier verbs. This avoids clip-dependent cubic
//! subdivision in the coverage rasterizer. Coverage is explicitly scanned at 8x
//! resolution (32x32 geometric sampling), then averaged to one coverage byte;
//! tiny-skia never composites artwork colors. Exposure retains HDR intermediates
//! through working output; only RGBA8 export clips and encodes the final color.
//! Unsupported document features are not represented.
//!
//! [`Renderer`] clones share one atomic allocation budget, including outstanding
//! [`Frame`] owners. Scratch is bounded by tile size and dependency depth, rather
//! than whole-page buffers for every layer. Per-request vector support bounds are
//! budgeted and discarded before returning the frame. Conservative leaf rejection
//! and proven full axis-aligned page/crop coverage avoid unnecessary evaluation;
//! all uncertain geometry and boundary coverage use the same reference rasterizer.
//! Invoke rendering on a worker thread;
//! it is synchronous and performs no scheduling, file I/O, or GPU work. The explicit
//! [`Renderer::new_retained`] constructor enables bounded immutable geometry and
//! layer-tile reuse; [`Renderer::new`] remains the uncached reference.
//!
//! ```
//! use affint_document::{Limits as DocumentLimits, PageSize, Session};
//! use affint_render::{FramePixels, Limits, OutputFormat, RenderRequest, Renderer, View};
//! use kurbo::Rect;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let session = Session::new(PageSize::new(64.0, 32.0)?, DocumentLimits::default())?;
//! let snapshot = session.snapshot();
//! let request = RenderRequest::new(snapshot.page_order()[0], 128, 64,
//!     View::Region(Rect::new(0.0, 0.0, 64.0, 32.0)))?;
//! let renderer = Renderer::new(Limits::default())?;
//! let frame = renderer.render(&snapshot, request)?;
//! assert_eq!(frame.rgba8().map(<[u8]>::len), Some(128 * 64 * 4));
//! assert_eq!(frame.revision(), snapshot.revision());
//! let working_request = request.with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?;
//! let working = renderer.render(&snapshot, working_request)?;
//! assert!(matches!(working.pixels(), FramePixels::LinearSrgbRgbaF32Premultiplied(_)));
//! assert!(working.rgba8().is_none());
//! assert_eq!(working.byte_len(), 128 * 64 * 16);
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod budget;
mod compositor;

use std::sync::Arc;

use affint_document::{AssetId, NodeId, PageId, Revision, Snapshot, Transform};
use kurbo::{Affine, Rect};

use budget::{Budget, Buffer};

/// Resource admission limits for one renderer and all of its clones.
///
/// Uncached source assets remain borrowed and charged by the document module.
/// Retention additionally charges each unique pinned original raster capacity once
/// within this renderer; this conservative overlap is explicit, not a process RSS
/// claim. No source pixels are copied. The byte budget includes returned
/// pixels, per-request vector support metadata, f32 tile buffers, coverage, and
/// conservative upstream path/scan workspace.
/// Allocator bookkeeping, thread stacks, and dependencies' fixed code/data are not
/// a measured RSS bound. tiny-skia's internal bounded allocations are infallible;
/// system-wide exhaustion there may still abort, despite prior byte admission.
/// Fixed renderer/cache synchronization and diagnostic controls are constant
/// bookkeeping; input-dependent cache containers and owner storage are admitted.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Maximum output width or height. Default: 16,384 pixels.
    pub max_dimension: u32,
    /// Maximum output pixels. Default: 32 million pixels.
    pub max_pixels: u64,
    /// Shared live allocation admission budget. Default: 512 MiB.
    pub max_bytes: usize,
    /// Color scratch tile side length, in `1..=2048`. Default: 256 pixels.
    /// Vector/crop coverage always uses an output-anchored 256-pixel grid, so
    /// changing this memory setting cannot change rendered artwork.
    pub tile_size: u32,
    /// Maximum active child/mask dependency depth, in `1..=128`. Default: 128.
    pub max_depth: usize,
    /// Maximum elements in one editable path. Default: 131,072.
    pub max_path_elements: usize,
    /// Maximum derived line/move/close elements from one device-space curve path.
    /// Default: 131,072. Original editable Béziers remain unchanged.
    pub max_flattened_elements: usize,
    /// Maximum node, operation, sample, and path-element work units per request.
    /// Default: twenty billion, including explicitly supersampled coverage work. This limits repeated evaluation of shared mask DAGs.
    pub max_work: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_dimension: 16_384,
            max_pixels: 32_000_000,
            max_bytes: 512 * 1024 * 1024,
            tile_size: 256,
            max_depth: 128,
            max_path_elements: 131_072,
            max_flattened_elements: 131_072,
            max_work: 20_000_000_000,
        }
    }
}

/// Explicit mapping from page coordinates to the exact output pixel grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    /// This finite positive-area page rectangle fills the output dimensions.
    /// Regions outside the page produce transparent pixels outside page coverage.
    Region(Rect),
    /// Explicit page-to-output placement. Integer coordinates denote pixel edges;
    /// samples are evaluated at `(x + 0.5, y + 0.5)`.
    Affine(Transform),
}

/// Explicit final pixel representation, independent of reference evaluation quality.
///
/// Both formats evaluate the same full-resolution document in premultiplied linear
/// sRGB f32. The format controls only output storage and final conversion; it is
/// part of request identity and must participate in scheduling and texture reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Tightly packed straight-alpha RGBA8 sRGB. HDR RGB clips at final encoding.
    /// RGB is canonical zero whenever the quantized alpha byte is zero.
    /// This is the default output and the explicit SDR export representation.
    SrgbRgba8Straight,
    /// Tightly packed `[R, G, B, A]` f32 arrays in premultiplied linear sRGB.
    /// RGB is finite working color and may exceed alpha or one. Alpha is in
    /// `[0, 1]`; zero-alpha pixels have canonical zero RGB. No transfer, clipping,
    /// or unpremultiplication is applied. Payload size is 16 bytes per pixel.
    LinearSrgbRgbaF32Premultiplied,
}

impl OutputFormat {
    const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::SrgbRgba8Straight => 4,
            Self::LinearSrgbRgbaF32Premultiplied => std::mem::size_of::<[f32; 4]>(),
        }
    }
}

/// A validated exact-size request with an explicit page-to-output mapping.
///
/// Precision is fixed to this crate's declared f32 reference policy. No implicit
/// reduced-quality request, source decimation, or fit-to-budget resize exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    page: PageId,
    width: u32,
    height: u32,
    view: View,
    format: OutputFormat,
}

impl RenderRequest {
    /// Creates an RGBA8 sRGB straight-alpha request, refusing zero dimensions,
    /// invalid regions, and byte arithmetic overflow. Configured resource and
    /// numerical limits are checked by [`Renderer::render`] before evaluation.
    pub fn new(page: PageId, width: u32, height: u32, view: View) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidRequest("output dimensions"));
        }
        output_bytes(width, height, OutputFormat::SrgbRgba8Straight)?;
        if let View::Region(region) = view {
            if ![region.x0, region.y0, region.x1, region.y1]
                .iter()
                .all(|value| value.is_finite())
                || !region.width().is_finite()
                || !region.height().is_finite()
                || region.width() <= 0.0
                || region.height() <= 0.0
            {
                return Err(Error::InvalidRequest("view region"));
            }
        }
        Ok(Self {
            page,
            width,
            height,
            view,
            format: OutputFormat::SrgbRgba8Straight,
        })
    }

    /// Selects the output representation, checking its 4/16-byte pixel arithmetic.
    ///
    /// This consumes the request and changes its identity. It does not reserve
    /// memory or change evaluation precision. Rendering may still refuse the
    /// configured byte budget rather than downgrade or resize the output.
    pub fn with_format(mut self, format: OutputFormat) -> Result<Self, Error> {
        output_bytes(self.width, self.height, format)?;
        self.format = format;
        Ok(self)
    }

    /// Explicit pixel representation included in this request's identity.
    pub const fn format(self) -> OutputFormat {
        self.format
    }

    /// Requested page identity.
    pub const fn page(self) -> PageId {
        self.page
    }
    /// Exact output width.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Exact output height.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Explicit mapping used by this request.
    pub const fn view(self) -> View {
        self.view
    }

    fn affine(self) -> Result<Affine, Error> {
        let affine = match self.view {
            View::Affine(transform) => transform.affine(),
            View::Region(region) => {
                let sx = f64::from(self.width) / region.width();
                let sy = f64::from(self.height) / region.height();
                Affine::new([sx, 0.0, 0.0, sy, -region.x0 * sx, -region.y0 * sy])
            }
        };
        compositor::validate_affine(affine, None)?;
        Ok(affine)
    }
}

/// Borrowed, exact-size pixels whose allocation remains owned by the frame.
///
/// Matching the enum makes transfer/alpha semantics explicit. These borrowed
/// views cannot outlive their frame and never detach its allocation lease.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FramePixels<'a> {
    /// Tightly packed straight-alpha RGBA8 sRGB bytes, four bytes per pixel.
    SrgbRgba8Straight(&'a [u8]),
    /// Tightly packed linear-sRGB premultiplied `[R, G, B, A]` f32 pixels.
    /// Finite HDR RGB is retained; no transfer, clipping or demultiplication.
    LinearSrgbRgbaF32Premultiplied(&'a [[f32; 4]]),
}

#[derive(Debug)]
enum FrameStorage {
    SrgbRgba8Straight(Buffer<u8>),
    LinearSrgbRgbaF32Premultiplied(Buffer<[f32; 4]>),
}

/// A completed exact-resolution result which owns one pixel allocation and lease.
///
/// Match [`Frame::pixels`] for the explicitly requested representation. Both
/// formats canonicalize zero-alpha RGB to zero; RGBA8 also canonicalizes positive
/// working alpha that rounds to a zero output alpha byte. Borrow pixels while
/// retaining the frame; there is deliberately no owned-pixels escape or deep `Clone`.
/// The lease outlives the renderer and releases only after pixel storage drops.
/// Move the frame or place it in `Arc<Frame>` for shared ownership.
#[derive(Debug)]
pub struct Frame {
    request: RenderRequest,
    revision: Revision,
    pixels: FrameStorage,
}

impl Frame {
    /// Borrows the requested representation without conversion or allocation.
    pub fn pixels(&self) -> FramePixels<'_> {
        match &self.pixels {
            FrameStorage::SrgbRgba8Straight(buffer) => FramePixels::SrgbRgba8Straight(&buffer.data),
            FrameStorage::LinearSrgbRgbaF32Premultiplied(buffer) => {
                FramePixels::LinearSrgbRgbaF32Premultiplied(&buffer.data)
            }
        }
    }
    /// Borrows RGBA8 output, or returns `None` for a working-f32 frame.
    /// No implicit conversion, empty stand-in, or format-mismatch panic occurs.
    pub fn rgba8(&self) -> Option<&[u8]> {
        match self.pixels() {
            FramePixels::SrgbRgba8Straight(pixels) => Some(pixels),
            FramePixels::LinearSrgbRgbaF32Premultiplied(_) => None,
        }
    }
    /// Exact pixel payload bytes, four or sixteen per requested output pixel.
    /// Allocation capacity may be larger and remains charged by the frame lease.
    pub fn byte_len(&self) -> usize {
        match self.pixels() {
            FramePixels::SrgbRgba8Straight(pixels) => std::mem::size_of_val(pixels),
            FramePixels::LinearSrgbRgbaF32Premultiplied(pixels) => std::mem::size_of_val(pixels),
        }
    }
    /// The evaluated snapshot revision; callers can discard obsolete worker results.
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    /// Exact request identity, including page, size, view, and output format.
    pub const fn request(&self) -> RenderRequest {
        self.request
    }
    /// Exact output width.
    pub const fn width(&self) -> u32 {
        self.request.width
    }
    /// Exact output height.
    pub const fn height(&self) -> u32 {
        self.request.height
    }
}

/// Synchronous toolkit-independent reference compositor.
///
/// Clones and concurrent calls share admission accounting. Separately constructed
/// renderers have separate budgets; applications should share/clone their worker
/// renderer when they intend a global render budget. Failures return no partial frame.
#[derive(Debug, Clone)]
pub struct Renderer {
    limits: Limits,
    budget: Arc<Budget>,
    retained: Option<Arc<compositor::RetainedCache>>,
}

/// Cumulative retained-work diagnostics, shared by cloned renderers.
///
/// Counts describe avoided evaluation, not measured latency. Resident admission
/// includes retained original-owner capacity and metadata as well as pixels.
/// Evicted entries still borrowed by a render remain charged until their last drop.
/// Fields are independent atomic observations, not a coordinated reservation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetainedStats {
    /// Successful reuse of an evaluated layer tile.
    pub tile_hits: u64,
    /// Layer tile lookups requiring ordinary evaluation.
    pub tile_misses: u64,
    /// Layer tiles entering ordinary evaluation (including conservative rejection).
    pub evaluated_tiles: u64,
    /// Reused exact device-space curve geometry or its preflight summary.
    pub geometry_hits: u64,
    /// Curve geometry lookups requiring ordinary preparation.
    pub geometry_misses: u64,
    /// Exact device-space curve buffers prepared on a cold lookup.
    pub prepared_curves: u64,
    /// Optional curve memo owners released to admit mandatory preflight workspace.
    pub preflight_reclaims: u64,
    /// Entries removed to admit other work; explicit clearing also counts.
    pub evictions: u64,
    /// Optional retention attempts refused by allocation or admission policy.
    pub refused: u64,
    /// Currently charged cache bytes, including active evicted owners.
    pub resident_bytes: usize,
    /// Capacity of unique original raster allocations currently pinned by retention.
    pub source_bytes: usize,
    /// Capacity bytes of retained evaluated tiles and prepared geometry.
    pub retained_pixel_bytes: usize,
}

impl Renderer {
    /// Creates a renderer, refusing zero limits, tiles above 2048, and depth above
    /// the implementation's hard recursion ceiling of 128.
    pub fn new(limits: Limits) -> Result<Self, Error> {
        if limits.max_dimension == 0
            || limits.max_pixels == 0
            || limits.max_bytes == 0
            || limits.tile_size == 0
            || limits.tile_size > 2048
            || limits.max_depth == 0
            || limits.max_depth > 128
            || limits.max_path_elements == 0
            || limits.max_flattened_elements == 0
            || limits.max_work == 0
        {
            return Err(Error::InvalidRequest("renderer limits"));
        }
        let budget = Arc::new(Budget::new(limits.max_bytes));
        Ok(Self {
            limits,
            budget,
            retained: None,
        })
    }

    /// Creates an exact renderer with a shared, byte-bounded retained-work cache.
    ///
    /// `cache_max_bytes` must be positive and no larger than `limits.max_bytes`.
    /// Both ceilings charge retained pixels, semantic metadata and original source
    /// owners; raster charges use allocation capacity, including spare capacity.
    /// Cache misses/refused retention run ordinary CPU evaluation. Unchanged layers
    /// reuse exact tiles across revisions; unchanged curves reuse prepared geometry.
    /// Translation reuse is conservatively limited to complete 256-pixel coverage
    /// blocks with proven identical line/raster evaluation mappings. Arbitrary zoom,
    /// fractional pan and translated curves may miss. No source/output reduction or
    /// approximate preview is performed. Dense final output is still written anew.
    pub fn new_retained(limits: Limits, cache_max_bytes: usize) -> Result<Self, Error> {
        if cache_max_bytes == 0 || cache_max_bytes > limits.max_bytes {
            return Err(Error::InvalidRequest("retained cache byte ceiling"));
        }
        let mut renderer = Self::new(limits)?;
        renderer.retained = Some(Arc::new(compositor::RetainedCache::new(
            Arc::clone(&renderer.budget),
            cache_max_bytes,
        )));
        Ok(renderer)
    }

    /// Returns cumulative cache diagnostics, or zeros for an uncached renderer.
    pub fn retained_stats(&self) -> RetainedStats {
        self.retained
            .as_ref()
            .map_or_else(RetainedStats::default, |cache| cache.stats())
    }

    /// Evicts retained entries and their map storage without invalidating frames.
    ///
    /// Clones share this operation. An entry already borrowed by a concurrent render
    /// stays immutable and charged until that borrower releases it. Completed frames
    /// own separate final output storage and retain their normal allocation lease.
    pub fn clear_retained(&self) {
        if let Some(cache) = &self.retained {
            cache.clear();
        }
    }

    /// Evaluates the page at the requested dimensions without reducing any source.
    ///
    /// Refuses missing references, resource/depth/work overruns, geometry outside
    /// the declared coverage range/precision, and nonfinite or underflowing color
    /// intermediates. RGBA8 output clips/encodes only at final conversion;
    /// working-f32 output preserves the unclipped premultiplied channels.
    pub fn render(&self, snapshot: &Snapshot, request: RenderRequest) -> Result<Frame, Error> {
        self.render_with_cancel(snapshot, request, || false)
    }

    /// Evaluates an exact request with cooperative cancellation for obsolete jobs.
    ///
    /// The callback returns `true` to cancel and is called at tile/node boundaries
    /// and at most every 4096 iterations of application pixel/path loops. It must
    /// be cheap and must not panic. Cancellation returns [`Error::Canceled`] and
    /// releases output/scratch admission. Complete immutable retained entries may
    /// remain after cancellation of a later part of the frame. No partial Frame is
    /// returned. tiny-skia's bounded synchronous coverage
    /// call is checked before/after and cannot be interrupted internally.
    pub fn render_with_cancel(
        &self,
        snapshot: &Snapshot,
        request: RenderRequest,
        canceled: impl Fn() -> bool,
    ) -> Result<Frame, Error> {
        compositor::render(self, snapshot, request, &canceled)
    }

    /// Shared charged bytes, including retained entries, outstanding frames and jobs.
    /// This is an atomic diagnostic snapshot, not a reservation guarantee.
    pub fn allocated_bytes(&self) -> usize {
        self.budget.used()
    }

    /// Admission settings used by this renderer and its clones.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
}

/// A render refusal. No variant yields a resized or partially evaluated result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A cooperative callback canceled the job; no partial frame is returned.
    #[error("render canceled")]
    Canceled,
    /// The request or renderer settings contain invalid dimensions or ranges.
    #[error("invalid {0}")]
    InvalidRequest(&'static str),
    /// The page does not exist in this snapshot.
    #[error("missing page {0:?}")]
    MissingPage(PageId),
    /// A referenced layer is absent.
    #[error("missing layer {0:?}")]
    MissingLayer(NodeId),
    /// A referenced original source is absent.
    #[error("missing raster asset {0:?}")]
    MissingAsset(AssetId),
    /// An input-derived length overflowed platform arithmetic.
    #[error("{0} overflow")]
    Overflow(&'static str),
    /// A per-request count/work resource exceeds its configured maximum.
    #[error("{resource} requires {actual}, limit {limit}")]
    LimitExceeded {
        /// Named count or work resource.
        resource: &'static str,
        /// Required amount.
        actual: u64,
        /// Configured maximum.
        limit: u64,
    },
    /// Concurrent jobs and held frames leave insufficient allocation budget.
    #[error("render allocation needs {requested} bytes with {used} of {limit} already charged")]
    BudgetExceeded {
        /// Additional requested bytes.
        requested: usize,
        /// Concurrent bytes observed at refusal.
        used: usize,
        /// Shared configured byte ceiling.
        limit: usize,
    },
    /// A fallible application allocation was refused by the system allocator.
    #[error("could not allocate {bytes} bytes for {resource}")]
    AllocationFailed {
        /// Purpose of the buffer.
        resource: &'static str,
        /// Requested byte count.
        bytes: usize,
    },
    /// A placement/point cannot meet the reference range or precision policy.
    #[error("{resource} is outside reference geometry range on {node:?}")]
    GeometryRange {
        /// Geometry or placement that could not be represented meaningfully.
        resource: &'static str,
        /// Layer responsible, or `None` for page/view geometry.
        node: Option<NodeId>,
    },
    /// Evaluation exceeded the finite nonzero f32 color range.
    #[error("{operation} exceeds f32 color range on {node:?}")]
    ColorRange {
        /// Layer responsible for the rejected intermediate.
        node: NodeId,
        /// Composition or live operation responsible.
        operation: &'static str,
    },
    /// A dependency exceeded the bounded evaluation depth.
    #[error("render dependency depth {actual} exceeds {limit} on {node:?}")]
    DepthExceeded {
        /// Layer whose evaluation would exceed the limit.
        node: NodeId,
        /// Requested depth, with page roots at one.
        actual: usize,
        /// Configured depth ceiling.
        limit: usize,
    },
    /// Validated, nondegenerate coverage geometry was refused by the rasterizer.
    #[error("coverage rasterizer refused geometry on {node:?}")]
    CoverageRejected {
        /// Layer responsible, or `None` for page clipping.
        node: Option<NodeId>,
    },
}

fn output_bytes(width: u32, height: u32, format: OutputFormat) -> Result<usize, Error> {
    let width = usize::try_from(width).map_err(|_| Error::Overflow("output width"))?;
    let height = usize::try_from(height).map_err(|_| Error::Overflow("output height"))?;
    width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(format.bytes_per_pixel()))
        .ok_or(Error::Overflow("output bytes"))
}
