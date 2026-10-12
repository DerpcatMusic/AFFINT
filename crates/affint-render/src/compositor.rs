use std::{cell::Cell, sync::Arc};

use affint_document::{Content, LiveOperation, MaskMode, NodeId, RasterAsset, Snapshot};
use kurbo::{Affine, CubicBez, PathEl, Point, Rect};
use palette::{
    LinSrgb, Mix, Srgb,
    blend::{Compose, PreAlpha},
};
use tiny_skia::{FillRule, IntSize, Mask, PathBuilder};

use crate::budget::{Budget, Buffer, Lease};
use crate::{Error, Frame, FrameStorage, OutputFormat, RenderRequest, Renderer, output_bytes};

mod retained;
mod spatial;
#[cfg(test)]
mod tests;

pub(crate) use retained::RetainedCache;
use retained::{Geometry, GeometryKey, TileKey, TilePixels};
use spatial::{VectorBounds, VectorSupports, raster_outside, rectangle_is_full};

type Pixel = PreAlpha<LinSrgb<f32>>;

#[derive(Clone, Copy)]
struct Optimizations {
    reject_leaves: bool,
    full_rectangles: bool,
}

impl Optimizations {
    const ENABLED: Self = Self {
        reject_leaves: true,
        full_rectangles: true,
    };

    #[cfg(test)]
    const DISABLED: Self = Self {
        reject_leaves: false,
        full_rectangles: false,
    };
}

// Coverage clipping changes tiny-skia's curve subdivision. Fix its output-space
// grid independently of color scratch tiles so memory configuration cannot change
// artwork semantics. This constant is part of the reference precision policy.
const COVERAGE_SIDE: u32 = 256;
// tiny-skia uses a 4x4 geometric sampling lattice. Explicit 8x coverage
// resolution raises this to 32x32 before final one-byte coverage quantization.
const COVERAGE_SCALE: u32 = 8;
const FLATTEN_TOLERANCE: f64 = 1.0 / 1024.0;
// Kurbo 0.13 flatten stores <=4096 (QuadBez, five-f64 parameters) entries
// for admitted cubics. This includes Vec growth and a conservative fixed margin.
const FLATTEN_WORKSPACE: usize = 2 * 1024 * 1024;
const MAX_FLATTEN_QUADS: usize = 4096;

fn transparent() -> Pixel {
    Pixel {
        color: LinSrgb::new(0.0, 0.0, 0.0),
        alpha: 0.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tile {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Tile {
    fn len(self) -> Result<usize, Error> {
        let width = usize::try_from(self.width).map_err(|_| Error::Overflow("tile width"))?;
        let height = usize::try_from(self.height).map_err(|_| Error::Overflow("tile height"))?;
        width
            .checked_mul(height)
            .ok_or(Error::Overflow("tile pixels"))
    }
}

pub(crate) fn render(
    renderer: &Renderer,
    snapshot: &Snapshot,
    request: RenderRequest,
    canceled: &dyn Fn() -> bool,
) -> Result<Frame, Error> {
    render_impl(
        renderer,
        snapshot,
        request,
        canceled,
        Optimizations::ENABLED,
    )
}

fn render_impl(
    renderer: &Renderer,
    snapshot: &Snapshot,
    request: RenderRequest,
    canceled: &dyn Fn() -> bool,
    optimizations: Optimizations,
) -> Result<Frame, Error> {
    check_cancel(canceled)?;
    let limits = &renderer.limits;
    check_limit(
        "output dimension",
        u64::from(request.width.max(request.height)),
        u64::from(limits.max_dimension),
    )?;
    let pixels = u64::from(request.width)
        .checked_mul(u64::from(request.height))
        .ok_or(Error::Overflow("output pixels"))?;
    check_limit("output pixels", pixels, limits.max_pixels)?;
    let page = snapshot
        .page(request.page)
        .ok_or(Error::MissingPage(request.page))?;
    let view = request.affine()?;
    let supports = match VectorSupports::new(snapshot, &renderer.budget, canceled) {
        Ok(supports) => supports,
        Err(Error::BudgetExceeded { .. }) if renderer.retained.is_some() => {
            renderer.clear_retained();
            VectorSupports::new(snapshot, &renderer.budget, canceled)?
        }
        Err(error) => return Err(error),
    };
    // Keep exact prepared geometry alive for all tiles of this request, including
    // a cold pan. Slots are bounded and charged; extreme pressure may fall back to
    // the ordinary preparation path rather than change output quality.
    let prepared = if renderer.retained.is_some() {
        let slots = supports.len().saturating_mul(2).min(4096);
        let bytes = slots
            .checked_mul(std::mem::size_of::<Option<Arc<Geometry>>>())
            .ok_or(Error::Overflow("prepared geometry slots"))?;
        match reserve_transient(renderer, bytes)
            .and_then(|lease| Buffer::with_lease(slots, None, "prepared geometry slots", lease))
        {
            Ok(prepared) => Some(prepared),
            Err(Error::BudgetExceeded { .. } | Error::AllocationFailed { .. }) => None,
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let mut evaluator = Evaluator {
        renderer,
        snapshot,
        canceled,
        work: 0,
        depth_peak: 0,
        elements_peak: 5,
        derived_bytes_peak: 0,
        flatten_workspace_peak: 0,
        output_width: request.width,
        output_height: request.height,
        scratch: Arc::clone(&renderer.budget),
        supports,
        prepared,
        optimizations,
    };
    for &node in page.roots() {
        evaluator.preflight(node, view, false, 1)?;
    }
    let tile_width = limits.tile_size.min(request.width);
    let tile_height = limits.tile_size.min(request.height);
    let tile_pixels = (tile_width as usize)
        .checked_mul(tile_height as usize)
        .ok_or(Error::Overflow("tile pixels"))?;
    let coverage_width = COVERAGE_SIDE.min(request.width);
    let coverage_height = COVERAGE_SIDE.min(request.height);
    let coverage_pixels = (coverage_width as usize)
        .checked_mul(coverage_height as usize)
        .ok_or(Error::Overflow("canonical coverage pixels"))?;
    let high_width = coverage_width
        .checked_mul(COVERAGE_SCALE)
        .ok_or(Error::Overflow("coverage sample width"))?;
    let high_pixels = coverage_pixels
        .checked_mul((COVERAGE_SCALE * COVERAGE_SCALE) as usize)
        .ok_or(Error::Overflow("coverage sample pixels"))?;
    let coverage_workspace = workspace_bytes(evaluator.elements_peak, high_width)?;
    let scratch_bytes = tile_pixels
        .checked_mul(std::mem::size_of::<Pixel>())
        .and_then(|bytes| bytes.checked_mul(evaluator.depth_peak + 1))
        .and_then(|bytes| bytes.checked_add(tile_pixels))
        .and_then(|bytes| bytes.checked_add(coverage_pixels))
        .and_then(|bytes| bytes.checked_add(high_pixels))
        .and_then(|bytes| bytes.checked_add(coverage_workspace))
        .and_then(|bytes| bytes.checked_add(evaluator.derived_bytes_peak))
        .and_then(|bytes| bytes.checked_add(evaluator.flatten_workspace_peak))
        .ok_or(Error::Overflow("peak render scratch"))?;
    let output_len = output_bytes(request.width, request.height, request.format)?;
    let peak_bytes = output_len
        .checked_add(scratch_bytes)
        .ok_or(Error::Overflow("peak render bytes"))?;
    // One atomic admission reserves all output/scratch before allocation; the
    // small preflight support table already holds a separately budgeted lease.
    // Local scratch leases stay within this reservation, and only output escapes
    // in Frame. Failed concurrent admissions never retain partially started jobs.
    let mut job = match renderer.budget.reserve(peak_bytes) {
        Ok(job) => job,
        Err(Error::BudgetExceeded { .. }) if renderer.retained.is_some() => {
            // Optional memo capacity itself must not strand bytes required by an
            // otherwise admitted uncached render. Drop both entries and slots.
            evaluator.prepared = None;
            renderer.clear_retained();
            renderer.budget.reserve(peak_bytes)?
        }
        Err(error) => return Err(error),
    };
    let output_lease = job.split(output_len)?;
    evaluator.scratch = Arc::new(Budget::new(scratch_bytes));
    let mut output = match request.format {
        OutputFormat::SrgbRgba8Straight => FrameStorage::SrgbRgba8Straight(Buffer::with_lease(
            output_len,
            0u8,
            "RGBA8 output",
            output_lease,
        )?),
        // output_len was checked in bytes; the typed array allocation has exactly
        // one element per pixel and retains that same complete output lease.
        OutputFormat::LinearSrgbRgbaF32Premultiplied => {
            FrameStorage::LinearSrgbRgbaF32Premultiplied(Buffer::with_lease(
                output_len / std::mem::size_of::<[f32; 4]>(),
                [0.0f32; 4],
                "linear RGBA f32 output",
                output_lease,
            )?)
        }
    };
    let page_bounds = Rect::new(0.0, 0.0, page.size().width(), page.size().height());
    let mut y = 0;
    while y < request.height {
        let height = limits.tile_size.min(request.height - y);
        let mut x = 0;
        while x < request.width {
            let width = limits.tile_size.min(request.width - x);
            check_cancel(canceled)?;
            let tile = Tile {
                x,
                y,
                width,
                height,
            };
            let mut result = evaluator.blank(tile)?;
            for &node in page.roots() {
                if let Some(source) = evaluator.node(node, view, false, 1, tile)? {
                    composite(&mut result.data, source.data(), node, canceled)?;
                }
            }
            let clip = evaluator.rect_coverage(page_bounds, view, tile, None)?;
            apply_rect_coverage(&mut result.data, clip, None, canceled)?;
            match &mut output {
                FrameStorage::SrgbRgba8Straight(buffer) => {
                    write_tile(
                        &mut buffer.data,
                        request.width,
                        tile,
                        &result.data,
                        canceled,
                    )?;
                }
                FrameStorage::LinearSrgbRgbaF32Premultiplied(buffer) => {
                    write_linear_tile(
                        &mut buffer.data,
                        request.width,
                        tile,
                        &result.data,
                        canceled,
                    )?;
                }
            }
            x += width;
        }
        y += height;
    }
    check_cancel(canceled)?;
    Ok(Frame {
        request,
        revision: snapshot.revision(),
        pixels: output,
    })
}

struct Evaluator<'a> {
    renderer: &'a Renderer,
    snapshot: &'a Snapshot,
    canceled: &'a dyn Fn() -> bool,
    work: u64,
    depth_peak: usize,
    elements_peak: usize,
    derived_bytes_peak: usize,
    flatten_workspace_peak: usize,
    output_width: u32,
    output_height: u32,
    scratch: Arc<Budget>,
    supports: VectorSupports,
    prepared: Option<Buffer<Option<Arc<Geometry>>>>,
    optimizations: Optimizations,
}

fn reserve_transient(renderer: &Renderer, bytes: usize) -> Result<Lease, Error> {
    match renderer.budget.reserve(bytes) {
        Ok(lease) => Ok(lease),
        Err(Error::BudgetExceeded { .. }) if renderer.retained.is_some() => {
            renderer.clear_retained();
            renderer.budget.reserve(bytes)
        }
        Err(error) => Err(error),
    }
}

impl Evaluator<'_> {
    fn charge_work(&mut self, amount: u64) -> Result<(), Error> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(Error::Overflow("render work"))?;
        check_limit("render work", self.work, self.renderer.limits.max_work)
    }

    fn depth(&self, node: NodeId, depth: usize) -> Result<(), Error> {
        if depth > self.renderer.limits.max_depth {
            return Err(Error::DepthExceeded {
                node,
                actual: depth,
                limit: self.renderer.limits.max_depth,
            });
        }
        Ok(())
    }

    // Reject excessive dependency depth before allocating any output. The document
    // module guarantees acyclicity. Bounded, precharged Kurbo work counts derived
    // geometry without owning a complete flattened path during this traversal.
    fn preflight(
        &mut self,
        id: NodeId,
        parent: Affine,
        mask_source: bool,
        depth: usize,
    ) -> Result<(), Error> {
        check_cancel(self.canceled)?;
        self.depth(id, depth)?;
        self.charge_work(1)?;
        let layer = self.snapshot.node(id).ok_or(Error::MissingLayer(id))?;
        if !mask_source && !layer.visible() {
            return Ok(());
        }
        self.depth_peak = self.depth_peak.max(depth);
        let placement = parent * layer.transform().affine();
        validate_affine(placement, Some(id))?;
        // These conversions previously happened after allocating/evaluating a
        // tile. Validate them even when a leaf will contribute exactly zero.
        color_scalar(layer.opacity(), id)?;
        match layer.content() {
            Content::Group => {
                for &child in layer.children() {
                    self.preflight(child, placement, false, depth + 1)?;
                }
            }
            Content::Raster { asset } => {
                self.snapshot
                    .asset(*asset)
                    .ok_or(Error::MissingAsset(*asset))?;
            }
            Content::Vector(vector) => {
                check_limit(
                    "path elements",
                    vector.path().elements().len() as u64,
                    self.renderer.limits.max_path_elements as u64,
                )?;
                vector_fill(vector.fill().components(), id)?;
                let elements = vector.path().elements();
                let mut support = VectorBounds::new(id);
                let count = if has_curves(elements) {
                    let cached = match self.prepared_geometry(id, placement) {
                        Some(geometry) => Some(geometry),
                        None => match &self.renderer.retained {
                            Some(cache) => cache.geometry(
                                id,
                                vector,
                                placement,
                                self.output_width,
                                self.output_height,
                                self.canceled,
                            )?,
                            None => None,
                        },
                    };
                    let count = if let Some(geometry) = cached {
                        support = geometry.support;
                        let count = geometry.path.data.len();
                        self.remember_geometry(geometry);
                        count
                    } else {
                        let _workspace = self.preflight_workspace()?;
                        let count = flatten_checked(
                            elements,
                            placement,
                            Some(id),
                            self.renderer.limits.max_flattened_elements,
                            self.canceled,
                            |_, element| {
                                support.include(
                                    element,
                                    Affine::IDENTITY,
                                    self.output_width,
                                    self.output_height,
                                );
                                Ok(())
                            },
                        )?;
                        if let Some(cache) = &self.renderer.retained {
                            let prepared = cache.retain_geometry(
                                GeometryKey {
                                    id,
                                    vector,
                                    placement,
                                    width: self.output_width,
                                    height: self.output_height,
                                },
                                count,
                                support,
                                self.canceled,
                                |destination| {
                                    flatten_checked(
                                        elements,
                                        placement,
                                        Some(id),
                                        self.renderer.limits.max_flattened_elements,
                                        self.canceled,
                                        |index, element| {
                                            *destination.get_mut(index).ok_or(
                                                Error::Overflow("prepared geometry count"),
                                            )? = element;
                                            Ok(())
                                        },
                                    )?;
                                    Ok(())
                                },
                            )?;
                            if let Some(prepared) = prepared {
                                self.charge_work(count as u64)?;
                                self.remember_geometry(prepared);
                            }
                        }
                        count
                    };
                    self.flatten_workspace_peak = FLATTEN_WORKSPACE;
                    self.derived_bytes_peak = self.derived_bytes_peak.max(
                        count
                            .checked_mul(std::mem::size_of::<PathEl>())
                            .ok_or(Error::Overflow("derived geometry bytes"))?,
                    );
                    count
                } else {
                    for (index, element) in elements.iter().copied().enumerate() {
                        periodic_cancel(self.canceled, index)?;
                        support.include(element, placement, self.output_width, self.output_height);
                    }
                    elements.len()
                };
                self.supports.observe(support)?;
                self.elements_peak = self.elements_peak.max(count);
                self.charge_work(count as u64)?;
            }
        }
        if let Some(mask) = layer.mask() {
            // Its source's normal owner is irrelevant: place it in TARGET local
            // coordinates, using the target placement and source affine exactly once.
            self.preflight(mask.source, placement, true, depth + 1)?;
        }
        Ok(())
    }

    fn blank(&self, tile: Tile) -> Result<Buffer<Pixel>, Error> {
        Buffer::new(
            &self.scratch,
            tile.len()?,
            transparent(),
            "linear RGBA tile",
        )
    }

    fn preflight_workspace(&mut self) -> Result<Lease, Error> {
        match reserve_transient(self.renderer, FLATTEN_WORKSPACE) {
            Ok(workspace) => Ok(workspace),
            Err(Error::BudgetExceeded { .. }) if self.prepared.is_some() => {
                // Earlier cold curves may still be held by the optional memo
                // after map eviction. Required count/support work takes priority.
                if let Some(cache) = &self.renderer.retained {
                    cache.reclaimed_preflight();
                }
                self.prepared = None;
                self.renderer.clear_retained();
                self.renderer.budget.reserve(FLATTEN_WORKSPACE)
            }
            Err(error) => Err(error),
        }
    }

    fn prepared_geometry(&self, id: NodeId, placement: Affine) -> Option<Arc<Geometry>> {
        self.prepared
            .as_ref()?
            .data
            .iter()
            .flatten()
            .find(|geometry| {
                geometry.id == id
                    && geometry.placement.as_coeffs().map(f64::to_bits)
                        == placement.as_coeffs().map(f64::to_bits)
            })
            .map(Arc::clone)
    }

    fn remember_geometry(&mut self, geometry: Arc<Geometry>) {
        if let Some(slot) = self
            .prepared
            .as_mut()
            .and_then(|prepared| prepared.data.iter_mut().find(|slot| slot.is_none()))
        {
            *slot = Some(geometry);
        }
    }

    fn node(
        &mut self,
        id: NodeId,
        parent: Affine,
        mask_source: bool,
        depth: usize,
        tile: Tile,
    ) -> Result<Option<TilePixels>, Error> {
        check_cancel(self.canceled)?;
        self.depth(id, depth)?;
        let layer = self.snapshot.node(id).ok_or(Error::MissingLayer(id))?;
        if !mask_source && !layer.visible() {
            return Ok(None);
        }
        if let Some(cache) = &self.renderer.retained {
            if let Some(pixels) = cache.tile(
                self.snapshot,
                TileKey {
                    id,
                    parent,
                    mask_source,
                    tile,
                    width: self.output_width,
                    height: self.output_height,
                },
                self.canceled,
            )? {
                check_cancel(self.canceled)?;
                return Ok(pixels
                    .pixels
                    .is_some()
                    .then_some(TilePixels::Retained(pixels)));
            }
            cache.evaluated();
        }
        self.charge_work(tile.len()? as u64)?;
        let placement = parent * layer.transform().affine();
        validate_affine(placement, Some(id))?;
        if self.optimizations.reject_leaves
            && self.leaf_outside(id, layer.content(), layer.operations(), placement, tile)?
        {
            // A mask is a separate target-local dependency. Preserve its error,
            // work and cancellation behavior even when its target is empty.
            if let Some(mask) = layer.mask() {
                self.node(mask.source, placement, true, depth + 1, tile)?;
            }
            if let Some(cache) = &self.renderer.retained {
                return cache.retain_tile(
                    self.snapshot,
                    TileKey {
                        id,
                        parent,
                        mask_source,
                        tile,
                        width: self.output_width,
                        height: self.output_height,
                    },
                    None,
                    self.canceled,
                );
            }
            return Ok(None);
        }
        let mut result = self.blank(tile)?;
        match layer.content() {
            Content::Group => {
                for &child in layer.children() {
                    if let Some(source) = self.node(child, placement, false, depth + 1, tile)? {
                        composite(&mut result.data, source.data(), child, self.canceled)?;
                    }
                }
            }
            Content::Raster { asset } => {
                self.charge_work(
                    (tile.len()? as u64)
                        .checked_mul(4)
                        .ok_or(Error::Overflow("raster samples"))?,
                )?;
                let asset = self
                    .snapshot
                    .asset(*asset)
                    .ok_or(Error::MissingAsset(*asset))?;
                raster(
                    &mut result.data,
                    asset,
                    placement.inverse(),
                    tile,
                    id,
                    self.canceled,
                )?;
            }
            Content::Vector(vector) => {
                let coverage =
                    self.coverage(vector.path().elements(), placement, tile, Some(id))?;
                let fill = vector_fill(vector.fill().components(), id)?;
                for (index, (pixel, amount)) in
                    result.data.iter_mut().zip(coverage.mask.data()).enumerate()
                {
                    periodic_cancel(self.canceled, index)?;
                    *pixel = fill;
                    multiply_coverage(pixel, f32::from(*amount) / 255.0, Some(id))?;
                }
            }
        }
        for operation in layer.operations() {
            self.charge_work(tile.len()? as u64)?;
            match operation {
                LiveOperation::Crop(rect) => {
                    let coverage = self.rect_coverage(*rect, placement, tile, Some(id))?;
                    apply_rect_coverage(&mut result.data, coverage, Some(id), self.canceled)?;
                }
                LiveOperation::Exposure(stops) => {
                    let gain = 2.0f64.powf(*stops) as f32;
                    if !gain.is_finite() || gain <= 0.0 {
                        return Err(Error::ColorRange {
                            node: id,
                            operation: "exposure gain",
                        });
                    }
                    for (index, pixel) in result.data.iter_mut().enumerate() {
                        periodic_cancel(self.canceled, index)?;
                        pixel.color.red = checked_scale(pixel.color.red, gain, id, "exposure")?;
                        pixel.color.green = checked_scale(pixel.color.green, gain, id, "exposure")?;
                        pixel.color.blue = checked_scale(pixel.color.blue, gain, id, "exposure")?;
                    }
                }
            }
        }
        if let Some(mask) = layer.mask() {
            let source = self.node(mask.source, placement, true, depth + 1, tile)?;
            for (index, pixel) in result.data.iter_mut().enumerate() {
                periodic_cancel(self.canceled, index)?;
                let source = source
                    .as_ref()
                    .map(|source| {
                        source
                            .data()
                            .get(index)
                            .copied()
                            .ok_or(Error::Overflow("mask source tile"))
                    })
                    .transpose()?;
                let coverage = match (source, mask.mode) {
                    // A rejected mask source is transparent; inversion happens
                    // afterward, exactly as for a rasterized all-zero source.
                    (None, _) => 0.0,
                    (Some(source), MaskMode::Alpha) => source.alpha,
                    // Source RGB is already premultiplied: this IS linear
                    // luminance times alpha, with no unstable unpremultiplication.
                    (Some(source), MaskMode::Luminance) => {
                        source.color.red * 0.2126
                            + source.color.green * 0.7152
                            + source.color.blue * 0.0722
                    }
                };
                if !coverage.is_finite() {
                    return Err(Error::ColorRange {
                        node: id,
                        operation: "mask luminance",
                    });
                }
                let coverage = coverage.clamp(0.0, 1.0);
                multiply_coverage(
                    pixel,
                    if mask.inverted {
                        1.0 - coverage
                    } else {
                        coverage
                    },
                    Some(id),
                )?;
            }
        }
        let opacity = color_scalar(layer.opacity(), id)?;
        for (index, pixel) in result.data.iter_mut().enumerate() {
            periodic_cancel(self.canceled, index)?;
            multiply_coverage(pixel, opacity, Some(id))?;
        }
        let result = match &self.renderer.retained {
            Some(cache) => cache.retain_tile(
                self.snapshot,
                TileKey {
                    id,
                    parent,
                    mask_source,
                    tile,
                    width: self.output_width,
                    height: self.output_height,
                },
                Some(result),
                self.canceled,
            )?,
            None => Some(TilePixels::Scratch(result)),
        };
        Ok(result)
    }

    fn leaf_outside(
        &self,
        id: NodeId,
        content: &Content,
        operations: &[LiveOperation],
        placement: Affine,
        tile: Tile,
    ) -> Result<bool, Error> {
        // Do not bypass a crop's existing numerical-range refusal. Uncertain
        // geometry uses the regular coverage path and its structured errors.
        if operations.iter().any(|operation| match operation {
            LiveOperation::Crop(rect) => !spatial::rectangle_geometry_safe(
                *rect,
                placement,
                self.output_width,
                self.output_height,
            ),
            LiveOperation::Exposure(_) => false,
        }) {
            return Ok(false);
        }
        match content {
            Content::Group => Ok(false),
            Content::Raster { asset } => {
                let asset = self
                    .snapshot
                    .asset(*asset)
                    .ok_or(Error::MissingAsset(*asset))?;
                Ok(raster_outside(asset, placement.inverse(), tile))
            }
            Content::Vector(_) => self.supports.outside(id, tile),
        }
    }

    fn rect_coverage(
        &mut self,
        rect: Rect,
        placement: Affine,
        tile: Tile,
        node: Option<NodeId>,
    ) -> Result<RectCoverage, Error> {
        check_cancel(self.canceled)?;
        if self.optimizations.full_rectangles
            && rectangle_is_full(rect, placement, tile, self.output_width, self.output_height)
        {
            return Ok(RectCoverage::Full);
        }
        let path = [
            PathEl::MoveTo(Point::new(rect.x0, rect.y0)),
            PathEl::LineTo(Point::new(rect.x1, rect.y0)),
            PathEl::LineTo(Point::new(rect.x1, rect.y1)),
            PathEl::LineTo(Point::new(rect.x0, rect.y1)),
            PathEl::ClosePath,
        ];
        self.coverage(&path, placement, tile, node)
            .map(RectCoverage::Sampled)
    }

    fn coverage(
        &mut self,
        elements: &[PathEl],
        placement: Affine,
        tile: Tile,
        node: Option<NodeId>,
    ) -> Result<Coverage, Error> {
        check_cancel(self.canceled)?;
        let retained = if has_curves(elements) {
            node.and_then(|id| self.prepared_geometry(id, placement))
        } else {
            None
        };
        let prepared = if has_curves(elements) && retained.is_none() {
            Some(self.prepare_geometry(elements, placement, node)?)
        } else {
            None
        };
        let (elements, placement) = match (&retained, &prepared) {
            (Some(path), _) => (path.path.data.as_slice(), Affine::IDENTITY),
            (_, Some(path)) => (path.data.as_slice(), Affine::IDENTITY),
            _ => (elements, placement),
        };
        if tile.x % COVERAGE_SIDE == 0
            && tile.y % COVERAGE_SIDE == 0
            && tile.width == COVERAGE_SIDE.min(self.output_width - tile.x)
            && tile.height == COVERAGE_SIDE.min(self.output_height - tile.y)
        {
            return self.coverage_block(elements, placement, tile, node);
        }
        let mut target = Buffer::new(&self.scratch, tile.len()?, 0u8, "assembled coverage tile")?;
        let right = tile
            .x
            .checked_add(tile.width)
            .ok_or(Error::Overflow("coverage tile right"))?;
        let bottom = tile
            .y
            .checked_add(tile.height)
            .ok_or(Error::Overflow("coverage tile bottom"))?;
        let mut block_y = tile.y / COVERAGE_SIDE * COVERAGE_SIDE;
        while block_y < bottom {
            let height = COVERAGE_SIDE.min(self.output_height - block_y);
            let mut block_x = tile.x / COVERAGE_SIDE * COVERAGE_SIDE;
            while block_x < right {
                let width = COVERAGE_SIDE.min(self.output_width - block_x);
                let block = Tile {
                    x: block_x,
                    y: block_y,
                    width,
                    height,
                };
                let coverage = self.coverage_block(elements, placement, block, node)?;
                let x0 = tile.x.max(block_x);
                let x1 = right.min(block_x + width);
                let y0 = tile.y.max(block_y);
                let y1 = bottom.min(block_y + height);
                let count = (x1 - x0) as usize;
                for (index, y) in (y0..y1).enumerate() {
                    periodic_cancel(self.canceled, index)?;
                    let source_offset = ((y - block_y) as usize)
                        .checked_mul(width as usize)
                        .and_then(|offset| offset.checked_add((x0 - block_x) as usize))
                        .ok_or(Error::Overflow("coverage source row"))?;
                    let target_offset = ((y - tile.y) as usize)
                        .checked_mul(tile.width as usize)
                        .and_then(|offset| offset.checked_add((x0 - tile.x) as usize))
                        .ok_or(Error::Overflow("coverage target row"))?;
                    let source_row = coverage
                        .mask
                        .data()
                        .get(source_offset..source_offset + count)
                        .ok_or(Error::Overflow("coverage source storage"))?;
                    let target_row = target
                        .data
                        .get_mut(target_offset..target_offset + count)
                        .ok_or(Error::Overflow("coverage target storage"))?;
                    target_row.copy_from_slice(source_row);
                }
                block_x += width;
            }
            block_y += height;
        }
        let size =
            IntSize::from_wh(tile.width, tile.height).ok_or(Error::CoverageRejected { node })?;
        let Buffer { data, lease } = target;
        let mask = Mask::from_vec(data, size).ok_or(Error::CoverageRejected { node })?;
        Ok(Coverage {
            mask,
            _lease: lease,
        })
    }

    fn prepare_geometry(
        &mut self,
        elements: &[PathEl],
        placement: Affine,
        node: Option<NodeId>,
    ) -> Result<Buffer<PathEl>, Error> {
        let _workspace = self.scratch.reserve(FLATTEN_WORKSPACE)?;
        let count = flatten_checked(
            elements,
            placement,
            node,
            self.renderer.limits.max_flattened_elements,
            self.canceled,
            |_, _| Ok(()),
        )?;
        self.charge_work(count as u64)?;
        let mut derived = Buffer::new(
            &self.scratch,
            count,
            PathEl::ClosePath,
            "derived device geometry",
        )?;
        flatten_checked(
            elements,
            placement,
            node,
            self.renderer.limits.max_flattened_elements,
            self.canceled,
            |index, element| {
                *derived
                    .data
                    .get_mut(index)
                    .ok_or(Error::Overflow("derived geometry count"))? = element;
                Ok(())
            },
        )?;
        self.charge_work(count as u64)?;
        Ok(derived)
    }

    fn coverage_block(
        &mut self,
        elements: &[PathEl],
        placement: Affine,
        tile: Tile,
        node: Option<NodeId>,
    ) -> Result<Coverage, Error> {
        check_limit(
            "path elements",
            elements.len() as u64,
            self.renderer
                .limits
                .max_flattened_elements
                .max(self.renderer.limits.max_path_elements) as u64,
        )?;
        self.charge_work(
            (elements.len() as u64)
                .checked_add(
                    (tile.len()? as u64)
                        .checked_mul(u64::from(COVERAGE_SCALE * COVERAGE_SCALE))
                        .ok_or(Error::Overflow("coverage sample work"))?,
                )
                .ok_or(Error::Overflow("coverage work"))?,
        )?;
        // Audit of tiny-skia 0.12: clipping emits <=18 edges per path edge;
        // edge Vec doubling + stable-sort workspace and path storage fit within
        // 8192 bytes per original element, with fixed/scanline allowances. This
        // is conservative admission for upstream infallible allocation, not RSS.
        check_cancel(self.canceled)?;
        let high_width = tile
            .width
            .checked_mul(COVERAGE_SCALE)
            .ok_or(Error::Overflow("coverage sample width"))?;
        let high_height = tile
            .height
            .checked_mul(COVERAGE_SCALE)
            .ok_or(Error::Overflow("coverage sample height"))?;
        let high_pixels = tile
            .len()?
            .checked_mul((COVERAGE_SCALE * COVERAGE_SCALE) as usize)
            .ok_or(Error::Overflow("coverage sample pixels"))?;
        let workspace = workspace_bytes(elements.len(), high_width)?;
        let _workspace = self.scratch.reserve(workspace)?;
        let points = elements
            .len()
            .checked_mul(3)
            .ok_or(Error::Overflow("coverage points"))?;
        let mut builder = PathBuilder::with_capacity(elements.len(), points);
        let mapping = Affine::scale(f64::from(COVERAGE_SCALE))
            * Affine::translate((-f64::from(tile.x), -f64::from(tile.y)))
            * placement;
        let mut draws = false;
        for (index, element) in elements.iter().enumerate() {
            periodic_cancel(self.canceled, index)?;
            match *element {
                PathEl::MoveTo(p) => {
                    let (x, y) = coverage_point(mapping * p, node)?;
                    builder.move_to(x, y);
                }
                PathEl::LineTo(p) => {
                    let (x, y) = coverage_point(mapping * p, node)?;
                    builder.line_to(x, y);
                    draws = true;
                }
                PathEl::QuadTo(p1, p2) => {
                    let (x1, y1) = coverage_point(mapping * p1, node)?;
                    let (x2, y2) = coverage_point(mapping * p2, node)?;
                    builder.quad_to(x1, y1, x2, y2);
                    draws = true;
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    let (x1, y1) = coverage_point(mapping * p1, node)?;
                    let (x2, y2) = coverage_point(mapping * p2, node)?;
                    let (x3, y3) = coverage_point(mapping * p3, node)?;
                    builder.cubic_to(x1, y1, x2, y2, x3, y3);
                    draws = true;
                }
                PathEl::ClosePath => builder.close(),
            }
        }
        let buffer = Buffer::new(
            &self.scratch,
            high_pixels,
            0u8,
            "supersampled coverage tile",
        )?;
        let size =
            IntSize::from_wh(high_width, high_height).ok_or(Error::CoverageRejected { node })?;
        let Buffer { data, lease } = buffer;
        let mut mask = Mask::from_vec(data, size).ok_or(Error::CoverageRejected { node })?;
        if draws {
            let path = builder.finish().ok_or(Error::CoverageRejected { node })?;
            mask.fill_path(
                &path,
                FillRule::Winding,
                true,
                tiny_skia::Transform::identity(),
            );
        }
        check_cancel(self.canceled)?;
        let high = Coverage {
            mask,
            _lease: lease,
        };
        let mut final_mask = Buffer::new(&self.scratch, tile.len()?, 0u8, "8-bit coverage tile")?;
        let divisor = COVERAGE_SCALE * COVERAGE_SCALE;
        for (index, amount) in final_mask.data.iter_mut().enumerate() {
            periodic_cancel(self.canceled, index)?;
            let x = index % tile.width as usize;
            let y = index / tile.width as usize;
            let mut sum = 0u32;
            for sy in 0..COVERAGE_SCALE as usize {
                let row = (y * COVERAGE_SCALE as usize + sy) * high_width as usize
                    + x * COVERAGE_SCALE as usize;
                let samples = high
                    .mask
                    .data()
                    .get(row..row + COVERAGE_SCALE as usize)
                    .ok_or(Error::Overflow("coverage reduction row"))?;
                sum += samples.iter().map(|sample| u32::from(*sample)).sum::<u32>();
            }
            *amount = ((sum + divisor / 2) / divisor) as u8;
        }
        let size =
            IntSize::from_wh(tile.width, tile.height).ok_or(Error::CoverageRejected { node })?;
        let Buffer { data, lease } = final_mask;
        let mask = Mask::from_vec(data, size).ok_or(Error::CoverageRejected { node })?;
        Ok(Coverage {
            mask,
            _lease: lease,
        })
    }
}

enum RectCoverage {
    Full,
    Sampled(Coverage),
}

fn apply_rect_coverage(
    pixels: &mut [Pixel],
    coverage: RectCoverage,
    node: Option<NodeId>,
    canceled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if let RectCoverage::Sampled(coverage) = coverage {
        for (index, (pixel, amount)) in pixels.iter_mut().zip(coverage.mask.data()).enumerate() {
            periodic_cancel(canceled, index)?;
            multiply_coverage(pixel, f32::from(*amount) / 255.0, node)?;
        }
    }
    check_cancel(canceled)
}

struct Coverage {
    mask: Mask,
    _lease: Lease,
}

pub(crate) fn validate_affine(affine: Affine, node: Option<NodeId>) -> Result<(), Error> {
    // Geometry placement stays f64; only device-space coverage points become f32.
    // Bounded forward/inverse coefficients exclude numerically extreme placement
    // while allowing ordinary large canvases, zoom, translations and shears.
    const MAX: f64 = 1_099_511_627_776.0; // 2^40
    if !affine.determinant().is_finite()
        || affine.determinant() == 0.0
        || affine
            .as_coeffs()
            .iter()
            .chain(affine.inverse().as_coeffs().iter())
            .any(|value| !value.is_finite() || value.abs() > MAX)
    {
        return Err(Error::GeometryRange {
            resource: "affine placement",
            node,
        });
    }
    Ok(())
}

fn coverage_point(point: Point, node: Option<NodeId>) -> Result<(f32, f32), Error> {
    fn component(value: f64, node: Option<NodeId>) -> Result<f32, Error> {
        let result = value as f32;
        if !value.is_finite()
            || value.abs() > 1_048_576.0
            || !result.is_finite()
            || (f64::from(result) - value).abs() > 1.0 / 64.0
            || (value != 0.0 && result == 0.0)
        {
            return Err(Error::GeometryRange {
                resource: "device coverage point",
                node,
            });
        }
        Ok(result)
    }
    Ok((component(point.x, node)?, component(point.y, node)?))
}

fn check_limit(resource: &'static str, actual: u64, limit: u64) -> Result<(), Error> {
    if actual > limit {
        return Err(Error::LimitExceeded {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

fn color_scalar(value: f64, node: NodeId) -> Result<f32, Error> {
    let result = value as f32;
    if !result.is_finite() || (value != 0.0 && result == 0.0) {
        return Err(Error::ColorRange {
            node,
            operation: "color/opacity conversion",
        });
    }
    Ok(result)
}

fn vector_fill([r, g, b, a]: [f64; 4], node: NodeId) -> Result<Pixel, Error> {
    let alpha = color_scalar(a, node)?;
    Ok(Pixel {
        color: LinSrgb::new(
            checked_scale(
                color_scalar(r, node)?,
                alpha,
                node,
                "fill premultiplication",
            )?,
            checked_scale(
                color_scalar(g, node)?,
                alpha,
                node,
                "fill premultiplication",
            )?,
            checked_scale(
                color_scalar(b, node)?,
                alpha,
                node,
                "fill premultiplication",
            )?,
        ),
        alpha,
    })
}

fn checked_scale(
    value: f32,
    gain: f32,
    node: NodeId,
    operation: &'static str,
) -> Result<f32, Error> {
    let result = value * gain;
    if !result.is_finite() || (value != 0.0 && gain != 0.0 && result == 0.0) {
        return Err(Error::ColorRange { node, operation });
    }
    Ok(result)
}

fn multiply_coverage(pixel: &mut Pixel, amount: f32, node: Option<NodeId>) -> Result<(), Error> {
    fn scale(value: f32, amount: f32, node: Option<NodeId>) -> Result<f32, Error> {
        let result = value * amount;
        if !result.is_finite() || (value != 0.0 && amount != 0.0 && result == 0.0) {
            return Err(match node {
                Some(node) => Error::ColorRange {
                    node,
                    operation: "coverage/opacity",
                },
                None => Error::GeometryRange {
                    resource: "page coverage color range",
                    node: None,
                },
            });
        }
        Ok(result)
    }
    pixel.color.red = scale(pixel.color.red, amount, node)?;
    pixel.color.green = scale(pixel.color.green, amount, node)?;
    pixel.color.blue = scale(pixel.color.blue, amount, node)?;
    pixel.alpha = scale(pixel.alpha, amount, node)?;
    Ok(())
}

fn composite(
    destination: &mut [Pixel],
    source: &[Pixel],
    node: NodeId,
    canceled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    for (index, (destination, source)) in destination.iter_mut().zip(source).enumerate() {
        periodic_cancel(canceled, index)?;
        *destination = source.over(*destination);
        if !destination.color.red.is_finite()
            || !destination.color.green.is_finite()
            || !destination.color.blue.is_finite()
            || !destination.alpha.is_finite()
        {
            return Err(Error::ColorRange {
                node,
                operation: "source-over",
            });
        }
    }
    Ok(())
}

fn raster(
    destination: &mut [Pixel],
    asset: &RasterAsset,
    inverse: Affine,
    tile: Tile,
    node: NodeId,
    canceled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let width = tile.width as usize;
    for (index, pixel) in destination.iter_mut().enumerate() {
        periodic_cancel(canceled, index)?;
        let point = inverse
            * Point::new(
                f64::from(tile.x) + (index % width) as f64 + 0.5,
                f64::from(tile.y) + (index / width) as f64 + 0.5,
            );
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(Error::GeometryRange {
                resource: "inverse source sample",
                node: Some(node),
            });
        }
        // Original sample centers are at .5. Identity placement must select the
        // exact original sample, rather than accidentally blend its neighbours.
        let x = point.x - 0.5;
        let y = point.y - 0.5;
        if x <= -1.0
            || y <= -1.0
            || x >= f64::from(asset.size().width())
            || y >= f64::from(asset.size().height())
        {
            continue;
        }
        let left = x.floor() as i64;
        let top = y.floor() as i64;
        let fx = (x - x.floor()) as f32;
        let fy = (y - y.floor()) as f32;
        let a = source_pixel(asset, left, top)?.mix(source_pixel(asset, left + 1, top)?, fx);
        let b =
            source_pixel(asset, left, top + 1)?.mix(source_pixel(asset, left + 1, top + 1)?, fx);
        *pixel = a.mix(b, fy);
    }
    Ok(())
}

fn source_pixel(asset: &RasterAsset, x: i64, y: i64) -> Result<Pixel, Error> {
    if x < 0
        || y < 0
        || x >= i64::from(asset.size().width())
        || y >= i64::from(asset.size().height())
    {
        return Ok(transparent());
    }
    let x = usize::try_from(x).map_err(|_| Error::Overflow("source coordinate"))?;
    let y = usize::try_from(y).map_err(|_| Error::Overflow("source coordinate"))?;
    let width =
        usize::try_from(asset.size().width()).map_err(|_| Error::Overflow("source width"))?;
    let offset = y
        .checked_mul(width)
        .and_then(|n| n.checked_add(x))
        .and_then(|n| n.checked_mul(4))
        .ok_or(Error::Overflow("source offset"))?;
    let rgba = asset
        .pixels()
        .get(offset..offset + 4)
        .ok_or(Error::Overflow("source pixels"))?;
    let linear: LinSrgb<f32> = Srgb::new(rgba[0], rgba[1], rgba[2]).into_linear();
    Ok(Pixel::new(linear, f32::from(rgba[3]) / 255.0))
}

fn write_tile(
    output: &mut [u8],
    output_width: u32,
    tile: Tile,
    pixels: &[Pixel],
    canceled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let output_width =
        usize::try_from(output_width).map_err(|_| Error::Overflow("output stride"))?;
    let width = tile.width as usize;
    for (index, pixel) in pixels.iter().enumerate() {
        periodic_cancel(canceled, index)?;
        let row = tile.y as usize + index / width;
        let column = tile.x as usize + index % width;
        let offset = row
            .checked_mul(output_width)
            .and_then(|n| n.checked_add(column))
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::Overflow("output offset"))?;
        let target = output
            .get_mut(offset..offset + 4)
            .ok_or(Error::Overflow("output storage"))?;
        let alpha = (pixel.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        // Canonical transparency belongs to the quantized representation too.
        // A positive working alpha can round to zero; do not retain colored RGB.
        if alpha == 0 {
            target.fill(0);
            continue;
        }
        // Clamp only at this explicitly lossy RGBA8 conversion. Clamping the
        // premultiplied numerator to alpha avoids overflow when demultiplying HDR.
        let linear = LinSrgb::new(
            pixel.color.red.clamp(0.0, pixel.alpha) / pixel.alpha,
            pixel.color.green.clamp(0.0, pixel.alpha) / pixel.alpha,
            pixel.color.blue.clamp(0.0, pixel.alpha) / pixel.alpha,
        );
        let encoded: Srgb<f32> = Srgb::from_linear(linear);
        let encoded: Srgb<u8> = encoded.into_format();
        target.copy_from_slice(&[encoded.red, encoded.green, encoded.blue, alpha]);
    }
    Ok(())
}

fn write_linear_tile(
    output: &mut [[f32; 4]],
    output_width: u32,
    tile: Tile,
    pixels: &[Pixel],
    canceled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let output_width =
        usize::try_from(output_width).map_err(|_| Error::Overflow("output stride"))?;
    let width = tile.width as usize;
    for (index, pixel) in pixels.iter().enumerate() {
        periodic_cancel(canceled, index)?;
        let row = tile.y as usize + index / width;
        let column = tile.x as usize + index % width;
        let offset = row
            .checked_mul(output_width)
            .and_then(|n| n.checked_add(column))
            .ok_or(Error::Overflow("output offset"))?;
        let target = output
            .get_mut(offset)
            .ok_or(Error::Overflow("output storage"))?;
        // Evaluation has already checked finite premultiplied intermediates.
        // Preserve them verbatim: transfer, RGB clipping and demultiplication
        // belong to explicit export/display conversion, not working storage.
        *target = if pixel.alpha == 0.0 {
            [0.0; 4]
        } else {
            [
                pixel.color.red,
                pixel.color.green,
                pixel.color.blue,
                pixel.alpha,
            ]
        };
    }
    Ok(())
}

fn workspace_bytes(elements: usize, width: u32) -> Result<usize, Error> {
    elements
        .checked_mul(8192)
        .and_then(|bytes| bytes.checked_add((width as usize + 1) * 64))
        .and_then(|bytes| bytes.checked_add(65_536))
        .ok_or(Error::Overflow("coverage workspace"))
}

fn check_cancel(canceled: &dyn Fn() -> bool) -> Result<(), Error> {
    if canceled() {
        return Err(Error::Canceled);
    }
    Ok(())
}

fn periodic_cancel(canceled: &dyn Fn() -> bool, index: usize) -> Result<(), Error> {
    if index % 4096 == 0 {
        check_cancel(canceled)?;
    }
    Ok(())
}

fn has_curves(elements: &[PathEl]) -> bool {
    elements
        .iter()
        .any(|element| matches!(element, PathEl::QuadTo(..) | PathEl::CurveTo(..)))
}

fn flatten_checked(
    elements: &[PathEl],
    placement: Affine,
    node: Option<NodeId>,
    limit: usize,
    canceled: &dyn Fn() -> bool,
    mut emit: impl FnMut(usize, PathEl) -> Result<(), Error>,
) -> Result<usize, Error> {
    // Validate before Kurbo derives subdivision counts or reserves its private Vec.
    // These point bounds make its quadratic workspace bound meaningful.
    let mut last = None;
    for (index, element) in elements.iter().copied().enumerate() {
        periodic_cancel(canceled, index)?;
        match placement * element {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => {
                coverage_point(p, node)?;
                last = Some(p);
            }
            PathEl::QuadTo(p1, p2) => {
                coverage_point(p1, node)?;
                coverage_point(p2, node)?;
                last = Some(p2);
            }
            PathEl::CurveTo(p1, p2, p3) => {
                coverage_point(p1, node)?;
                coverage_point(p2, node)?;
                coverage_point(p3, node)?;
                if let Some(p0) = last {
                    // Pinning the same 0.1 cubic-to-quadratic tolerance fraction as
                    // audited Kurbo flatten lets admission bound its private buffer.
                    let quads = CubicBez::new(p0, p1, p2, p3)
                        .to_quads(FLATTEN_TOLERANCE * 0.1)
                        .size_hint()
                        .0;
                    check_limit(
                        "curve flatten quadratics",
                        quads as u64,
                        MAX_FLATTEN_QUADS as u64,
                    )?;
                }
                last = Some(p3);
            }
            PathEl::ClosePath => last = None,
        }
    }
    let stopped = Cell::new(false);
    let mut failure = None;
    let mut count = 0usize;
    kurbo::flatten(
        elements
            .iter()
            .copied()
            .map(|element| placement * element)
            .take_while(|_| !stopped.get()),
        FLATTEN_TOLERANCE,
        |element| {
            if failure.is_some() {
                return;
            }
            let accepted = (|| {
                periodic_cancel(canceled, count)?;
                let next = count
                    .checked_add(1)
                    .ok_or(Error::Overflow("flattened path count"))?;
                check_limit("flattened path elements", next as u64, limit as u64)?;
                match element {
                    PathEl::MoveTo(point) | PathEl::LineTo(point) => {
                        coverage_point(point, node)?;
                    }
                    PathEl::ClosePath => {}
                    _ => return Err(Error::CoverageRejected { node }),
                }
                emit(count, element)?;
                count = next;
                Ok(())
            })();
            if let Err(error) = accepted {
                failure = Some(error);
                stopped.set(true);
            }
        },
    );
    if let Some(error) = failure {
        return Err(error);
    }
    check_cancel(canceled)?;
    Ok(count)
}
