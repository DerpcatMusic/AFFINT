//! Conservative support proofs, never substitutes for reference coverage at edges.

use std::sync::Arc;

use affint_document::{Content, NodeId, RasterAsset, Snapshot};
use kurbo::{Affine, PathEl, Point, Rect};

use super::{COVERAGE_SCALE, COVERAGE_SIDE, Tile, coverage_point, periodic_cancel};
use crate::{
    Error,
    budget::{Budget, Buffer},
};

pub(super) struct VectorSupports {
    entries: Buffer<Option<VectorBounds>>,
}

impl VectorSupports {
    pub(super) fn len(&self) -> usize {
        self.entries.data.len()
    }
    pub(super) fn new(
        snapshot: &Snapshot,
        budget: &Arc<Budget>,
        canceled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let mut count = 0usize;
        for (index, (_, layer)) in snapshot.layers().enumerate() {
            periodic_cancel(canceled, index)?;
            if matches!(layer.content(), Content::Vector(_)) {
                count = count
                    .checked_add(1)
                    .ok_or(Error::Overflow("vector bounds count"))?;
            }
        }
        // Identity-sorted entries share one allocation/lease. Collecting bounds
        // adds no retained path and no persistent cache or snapshot ownership.
        let mut entries = Buffer::new(budget, count, None, "vector support bounds")?;
        let mut slot = 0;
        for (index, (id, layer)) in snapshot.layers().enumerate() {
            periodic_cancel(canceled, index)?;
            if matches!(layer.content(), Content::Vector(_)) {
                *entries
                    .data
                    .get_mut(slot)
                    .ok_or(Error::Overflow("vector bounds count"))? = Some(VectorBounds::new(*id));
                slot += 1;
            }
        }
        Ok(Self { entries })
    }

    fn index(&self, node: NodeId) -> Result<usize, Error> {
        self.entries
            .data
            .binary_search_by_key(&Some(node), |entry| {
                entry.as_ref().map(|bounds| bounds.node)
            })
            .map_err(|_| Error::Overflow("vector bounds lookup"))
    }

    pub(super) fn observe(&mut self, bounds: VectorBounds) -> Result<(), Error> {
        let index = self.index(bounds.node)?;
        self.entries
            .data
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(Error::Overflow("vector bounds entry"))?
            .merge(bounds);
        Ok(())
    }

    pub(super) fn outside(&self, node: NodeId, tile: Tile) -> Result<bool, Error> {
        let index = self.index(node)?;
        Ok(self
            .entries
            .data
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Error::Overflow("vector bounds entry"))?
            .outside(tile))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VectorBounds {
    node: NodeId,
    hull: Option<Rect>,
    safe: bool,
}

impl VectorBounds {
    pub(super) const fn new(node: NodeId) -> Self {
        Self {
            node,
            hull: None,
            safe: true,
        }
    }

    pub(super) fn include(&mut self, element: PathEl, placement: Affine, width: u32, height: u32) {
        match element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => {
                self.safe &= geometry_safe(point, placement, width, height);
                let point = placement * point;
                if point.x.is_finite() && point.y.is_finite() {
                    let rect = Rect::new(point.x, point.y, point.x, point.y);
                    self.hull = Some(self.hull.map_or(rect, |hull| hull.union(rect)));
                } else {
                    self.safe = false;
                }
            }
            PathEl::ClosePath => {}
            // Curve bounds are collected from the exact derived lines emitted by
            // preflight, never from an assumed flattening-error/control-box bound.
            PathEl::QuadTo(..) | PathEl::CurveTo(..) => self.safe = false,
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.safe &= other.safe;
        if let Some(rect) = other.hull {
            self.hull = Some(self.hull.map_or(rect, |hull| hull.union(rect)));
        }
    }

    pub(super) fn outside(&self, tile: Tile) -> bool {
        if !self.safe {
            return false;
        }
        let Some(hull) = self.hull else {
            return true;
        };
        // One output pixel exceeds float conversion, scan edge quantization and
        // AA support together. Strict comparisons retain all boundary cases.
        hull.x1 + 1.0 < f64::from(tile.x)
            || hull.y1 + 1.0 < f64::from(tile.y)
            || hull.x0 - 1.0 > f64::from(tile.x + tile.width)
            || hull.y0 - 1.0 > f64::from(tile.y + tile.height)
    }
}

// A sufficient domain for every canonical block's existing coverage_point check.
// The maximum component is 2^17 high-resolution samples, whose f32 rounding
// error is <=1/128 sample, smaller than the existing 1/64 limit. Restricting every
// nonzero term to >=1e-20 also makes f64 cancellation unable to produce a nonzero
// value below f32's representable range. Uncertain inputs fall back, not refuse.
pub(super) fn geometry_safe(point: Point, placement: Affine, width: u32, height: u32) -> bool {
    let [a, b, c, d, e, f] = placement.as_coeffs();
    let scale = f64::from(COVERAGE_SCALE);
    let max_x = f64::from((width - 1) / COVERAGE_SIDE * COVERAGE_SIDE) * scale;
    let max_y = f64::from((height - 1) / COVERAGE_SIDE * COVERAGE_SIDE) * scale;
    fn component(terms: [f64; 3], anchor: f64) -> bool {
        terms
            .iter()
            .all(|term| term.is_finite() && (*term == 0.0 || term.abs() >= 1e-20))
            && terms.iter().map(|term| term.abs()).sum::<f64>() + anchor <= 131_072.0
    }
    component([scale * a * point.x, scale * c * point.y, scale * e], max_x)
        && component([scale * b * point.x, scale * d * point.y, scale * f], max_y)
}

fn corners(rect: Rect) -> [Point; 4] {
    [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ]
}

pub(super) fn rectangle_geometry_safe(
    rect: Rect,
    placement: Affine,
    width: u32,
    height: u32,
) -> bool {
    corners(rect)
        .into_iter()
        .all(|point| geometry_safe(point, placement, width, height))
}

pub(super) fn raster_outside(asset: &RasterAsset, inverse: Affine, tile: Tile) -> bool {
    // The affine extrema occur at corners of the destination sample-center
    // rectangle. Check in source space so even a highly sheared support polygon
    // cannot lose the bilinear transparent-border influence at +/- half a pixel.
    let rect = Rect::new(
        f64::from(tile.x) + 0.5,
        f64::from(tile.y) + 0.5,
        f64::from(tile.x + tile.width) - 0.5,
        f64::from(tile.y + tile.height) - 0.5,
    );
    let points = corners(rect).map(|point| inverse * point);
    let [a, b, c, d, e, f] = inverse.as_coeffs();
    let x_abs = rect.x0.abs().max(rect.x1.abs());
    let y_abs = rect.y0.abs().max(rect.y1.abs());
    // Enclose f64 multiply/add roundoff of all samples, including extrema whose
    // individual rounding differs from a corner. Decline nonfinite bounds.
    let x_guard = 16.0 * f64::EPSILON * (a.abs() * x_abs + c.abs() * y_abs + e.abs() + 1.0);
    let y_guard = 16.0 * f64::EPSILON * (b.abs() * x_abs + d.abs() * y_abs + f.abs() + 1.0);
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min)
        - x_guard;
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max)
        + x_guard;
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min)
        - y_guard;
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max)
        + y_guard;
    [min_x, max_x, min_y, max_y]
        .iter()
        .all(|value| value.is_finite())
        && (max_x < -0.5
            || max_y < -0.5
            || min_x > f64::from(asset.size().width()) + 0.5
            || min_y > f64::from(asset.size().height()) + 0.5)
}

pub(super) fn rectangle_is_full(
    rect: Rect,
    placement: Affine,
    tile: Tile,
    width: u32,
    height: u32,
) -> bool {
    let [_, b, c, _, _, _] = placement.as_coeffs();
    // Rotated/sheared bounding rectangles do not prove polygon containment.
    // This increment deliberately handles only axis-aligned transformed polygons.
    if b != 0.0 || c != 0.0 || !rectangle_geometry_safe(rect, placement, width, height) {
        return false;
    }
    let right = tile.x + tile.width;
    let bottom = tile.y + tile.height;
    let mut y = tile.y / COVERAGE_SIDE * COVERAGE_SIDE;
    while y < bottom {
        let block_height = COVERAGE_SIDE.min(height - y);
        let mut x = tile.x / COVERAGE_SIDE * COVERAGE_SIDE;
        while x < right {
            let block_width = COVERAGE_SIDE.min(width - x);
            // Use the identical f64 composition and f32 conversion as coverage,
            // on complete canonical blocks even for a 17-pixel color tile.
            let mapping = Affine::scale(f64::from(COVERAGE_SCALE))
                * Affine::translate((-f64::from(x), -f64::from(y)))
                * placement;
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for point in corners(rect) {
                let Ok((px, py)) = coverage_point(mapping * point, None) else {
                    return false;
                };
                min_x = min_x.min(px);
                max_x = max_x.max(px);
                min_y = min_y.min(py);
                max_y = max_y.max(py);
            }
            // For an axis-aligned rectangle these extrema ARE its four edges,
            // including negative scales/winding. Every sample is strictly inside
            // or on the image boundary, so every coverage byte is exactly 255.
            if min_x > 0.0
                || min_y > 0.0
                || max_x < (block_width * COVERAGE_SCALE) as f32
                || max_y < (block_height * COVERAGE_SCALE) as f32
            {
                return false;
            }
            x += block_width;
        }
        y += block_height;
    }
    true
}
