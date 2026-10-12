//! Camera identity is separate from the immutable request that evaluated pixels.
use affint_document::{Revision, Transform};
use affint_render::{OutputFormat, RenderRequest, View};
use kurbo::{Affine, Point, Rect};

use super::Viewport;
use crate::{editor::AppResult, upload::Key};

pub const OVERSCAN: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraIntent {
    revision: Revision,
    request: RenderRequest,
    viewport: Viewport,
    scale: f64,
}
impl CameraIntent {
    pub fn new(
        revision: Revision,
        request: RenderRequest,
        viewport: Viewport,
        scale: f64,
    ) -> AppResult<Self> {
        if request.width() != viewport.width
            || request.height() != viewport.height
            || request.format() != OutputFormat::LinearSrgbRgbaF32Premultiplied
            || !scale.is_finite()
            || scale <= 0.
        {
            return Err("The requested camera dimensions/format/scale are invalid.".into());
        }
        affine(request)?;
        Ok(Self {
            revision,
            request,
            viewport,
            scale,
        })
    }
    pub fn request(self) -> RenderRequest {
        self.request
    }
    pub fn revision(self) -> Revision {
        self.revision
    }
    pub fn viewport(self) -> Viewport {
        self.viewport
    }
}

/// Native visible region and integer padding, bound to the actual evaluated Key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Evaluation {
    pub key: Key,
    pub camera: CameraIntent,
    pub margin: u32,
}
impl Evaluation {
    pub fn new(key: Key, camera: CameraIntent, margin: u32) -> AppResult<Self> {
        let padding = margin.checked_mul(2).ok_or("Camera padding overflow.")?;
        let expected = padded_request(camera.request, margin)?;
        if key.revision != camera.revision
            || key.request != expected
            || key.request.width()
                != camera
                    .viewport
                    .width
                    .checked_add(padding)
                    .ok_or("Camera width overflow.")?
            || key.request.height()
                != camera
                    .viewport
                    .height
                    .checked_add(padding)
                    .ok_or("Camera height overflow.")?
        {
            return Err("Canvas evaluation metadata does not match its immutable request.".into());
        }
        Ok(Self {
            key,
            camera,
            margin,
        })
    }
}
pub fn padded_request(request: RenderRequest, margin: u32) -> AppResult<RenderRequest> {
    let padding = margin.checked_mul(2).ok_or("Camera padding overflow.")?;
    Ok(RenderRequest::new(
        request.page(),
        request
            .width()
            .checked_add(padding)
            .ok_or("Camera width overflow.")?,
        request
            .height()
            .checked_add(padding)
            .ok_or("Camera height overflow.")?,
        View::Affine(Transform::new(
            (Affine::translate((f64::from(margin), f64::from(margin))) * affine(request)?)
                .as_coeffs(),
        )?),
    )?
    .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?)
}
/// Prefetch never steals the visible viewport's mandatory density. The actual
/// renderer and upload admissions still own scratch/cache and live-pair checks.
pub(crate) fn prefetch_fits(
    request: RenderRequest,
    limits: &affint_render::Limits,
    gpu_dimension: u32,
) -> bool {
    let pixels = u64::from(request.width()) * u64::from(request.height());
    request.width() <= limits.max_dimension.min(gpu_dimension)
        && request.height() <= limits.max_dimension.min(gpu_dimension)
        && pixels <= limits.max_pixels
        && pixels
            .checked_mul(32)
            .is_some_and(|bytes| bytes <= limits.max_bytes as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    EvaluatedSource,
    RetainedAtNativeDensity,
    PendingCoverage,
}

/// A plan is metadata only. It neither relabels nor owns another pixel buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayPlan {
    pub evaluation: Evaluation,
    pub viewport: Viewport,
    pub paint_size: [u32; 2],
    pub source_offset: [i32; 2],
    pub document_to_viewport: Transform,
    pub artboard: Rect,
    pub scale: f64,
    pub mode: DisplayMode,
    pub pending: bool,
}
/// The host supplies the exact candidate/current it will sample before authoring
/// chrome. Metadata is bounded to the latest work and the last presented source.
pub(super) fn select_plan(
    key: Key,
    artboard: Rect,
    latest: Option<Evaluation>,
    receipt: Option<DisplayPlan>,
    generation: u64,
    intent: CameraIntent,
) -> AppResult<Option<DisplayPlan>> {
    if key.generation != generation || key.request.page() != intent.request.page() {
        return Ok(None);
    }
    let evaluation = latest
        .filter(|evaluation| evaluation.key == key)
        .or_else(|| {
            receipt
                .map(|plan| plan.evaluation)
                .filter(|evaluation| evaluation.key == key)
        })
        .ok_or("The sampled canvas has no matching camera metadata.")?;
    Ok(Some(DisplayPlan::choose(evaluation, artboard, intent)?))
}
impl DisplayPlan {
    pub(crate) fn choose(
        evaluation: Evaluation,
        source_artboard: Rect,
        intent: CameraIntent,
    ) -> AppResult<Self> {
        if evaluation.key.request.page() != intent.request.page() {
            return Err("A retained canvas belongs to another page.".into());
        }
        if ![
            source_artboard.x0,
            source_artboard.y0,
            source_artboard.x1,
            source_artboard.y1,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err("The retained artboard has invalid camera coordinates.".into());
        }
        let source = affine(evaluation.key.request)?;
        let wanted = affine(intent.request)?;
        let source_size = [
            evaluation.key.request.width(),
            evaluation.key.request.height(),
        ];
        let target_size = [intent.viewport.width, intent.viewport.height];
        let baseline = i32::try_from(evaluation.margin)?;
        let mut offset = [baseline; 2];
        let mut paint_size = [
            target_size[0].min(source_size[0]),
            target_size[1].min(source_size[1]),
        ];
        let mut mode = DisplayMode::PendingCoverage;
        if let Some(translation) = integer_offset(source, wanted, target_size)
            && covered(source_artboard, source_size, translation, target_size)
            && target_size[0] <= source_size[0]
            && target_size[1] <= source_size[1]
        {
            offset = translation;
            paint_size = target_size;
            mode = DisplayMode::RetainedAtNativeDensity;
        } else if !covered(source_artboard, source_size, offset, paint_size) {
            // Resize may expose unrendered strips. Keep the previous evaluated
            // camera and its visible region, without stretching or invented pixels.
            paint_size = [
                target_size[0].min(evaluation.camera.viewport.width),
                target_size[1].min(evaluation.camera.viewport.height),
            ];
        }
        let shift = Affine::translate((-f64::from(offset[0]), -f64::from(offset[1])));
        let document_to_viewport = Transform::new((shift * source).as_coeffs())?;
        let artboard = shift.transform_rect_bbox(source_artboard);
        let pending = evaluation.key.revision != intent.revision
            || evaluation.camera != intent
            || offset != [baseline; 2]
            || paint_size != target_size;
        if !pending {
            mode = DisplayMode::EvaluatedSource;
        }
        Ok(Self {
            evaluation,
            viewport: intent.viewport,
            paint_size,
            source_offset: offset,
            document_to_viewport,
            artboard,
            scale: intent.scale,
            mode,
            pending,
        })
    }
    pub fn document_to_window(self) -> Affine {
        Affine::translate((f64::from(self.viewport.x), f64::from(self.viewport.y)))
            * self.document_to_viewport.affine()
    }
    pub fn document_point(self, logical: Point) -> Option<Point> {
        let physical = Point::new(logical.x * self.scale, logical.y * self.scale);
        let local = Point::new(
            physical.x - f64::from(self.viewport.x),
            physical.y - f64::from(self.viewport.y),
        );
        (local.x >= 0.
            && local.y >= 0.
            && local.x < f64::from(self.paint_size[0])
            && local.y < f64::from(self.paint_size[1]))
        .then(|| self.document_to_viewport.affine().inverse() * local)
    }
    pub fn logical_affine(self, viewport_origin: Point) -> Affine {
        Affine::translate((-viewport_origin.x, -viewport_origin.y))
            * Affine::scale(1. / self.scale)
            * self.document_to_window()
    }
    pub fn zoom(self) -> f64 {
        self.document_to_viewport.affine().as_coeffs()[0].abs() / self.scale
    }
    pub fn center(self, logical_viewport: mui::layout::Frame) -> Point {
        self.document_to_window().inverse()
            * Point::new(
                (logical_viewport.x + logical_viewport.size.width / 2.) * self.scale,
                (logical_viewport.y + logical_viewport.size.height / 2.) * self.scale,
            )
    }
    pub fn coverage_margin(self) -> f64 {
        let width = f64::from(self.evaluation.key.request.width());
        let height = f64::from(self.evaluation.key.request.height());
        f64::from(self.source_offset[0])
            .min(f64::from(self.source_offset[1]))
            .min(width - f64::from(self.source_offset[0]) - f64::from(self.paint_size[0]))
            .min(height - f64::from(self.source_offset[1]) - f64::from(self.paint_size[1]))
    }
}

fn affine(request: RenderRequest) -> AppResult<Affine> {
    match request.view() {
        View::Affine(transform) => Ok(transform.affine()),
        View::Region(region) => {
            let sx = f64::from(request.width()) / region.width();
            let sy = f64::from(request.height()) / region.height();
            Ok(Transform::new([sx, 0., 0., sy, -region.x0 * sx, -region.y0 * sy])?.affine())
        }
    }
}
fn integer_offset(source: Affine, target: Affine, size: [u32; 2]) -> Option<[i32; 2]> {
    let old = source.as_coeffs();
    let new = target.as_coeffs();
    if old[..4] != new[..4] {
        return None;
    }
    let offset = [old[4] - new[4], old[5] - new[5]];
    offset
        .into_iter()
        .zip(size)
        .all(|(value, size)| {
            value.is_finite()
                && value.fract() == 0.
                && value >= f64::from(i32::MIN)
                && value <= f64::from(i32::MAX) - f64::from(size)
        })
        .then(|| offset.map(|value| value as i32))
}
/// Pixels intersecting AA page support need real source coverage, including
/// centers just outside the page. Only the complementary region is proven zero.
fn covered(page: Rect, source: [u32; 2], offset: [i32; 2], target: [u32; 2]) -> bool {
    let required = Rect::new(
        (page.x0 - f64::from(offset[0])).floor(),
        (page.y0 - f64::from(offset[1])).floor(),
        (page.x1 - f64::from(offset[0])).ceil(),
        (page.y1 - f64::from(offset[1])).ceil(),
    )
    .intersect(Rect::new(
        0.,
        0.,
        f64::from(target[0]),
        f64::from(target[1]),
    ));
    required.is_zero_area()
        || (required.x0 >= -f64::from(offset[0])
            && required.y0 >= -f64::from(offset[1])
            && required.x1 <= f64::from(source[0]) - f64::from(offset[0])
            && required.y1 <= f64::from(source[1]) - f64::from(offset[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Editor;

    fn camera(
        editor: &Editor,
        viewport: Viewport,
        scale: f64,
        transform: Affine,
    ) -> AppResult<CameraIntent> {
        CameraIntent::new(
            editor.snapshot().revision(),
            RenderRequest::new(
                editor.page()?,
                viewport.width,
                viewport.height,
                View::Affine(Transform::new(transform.as_coeffs())?),
            )?
            .with_format(OutputFormat::LinearSrgbRgbaF32Premultiplied)?,
            viewport,
            scale,
        )
    }
    fn evaluation(camera: CameraIntent, margin: u32) -> AppResult<Evaluation> {
        Evaluation::new(
            Key {
                revision: camera.revision,
                request: padded_request(camera.request, margin)?,
                generation: 7,
                serial: 11,
            },
            camera,
            margin,
        )
    }
    #[test]
    fn native_translation_preserves_source_identity_and_pending_reference() -> AppResult<()> {
        let editor = Editor::demo()?;
        let viewport = Viewport {
            x: 40,
            y: 170,
            width: 80,
            height: 60,
        };
        let initial = camera(
            &editor,
            viewport,
            1.5,
            Affine::new([1.5, 0., 0., 1.5, -300., -150.]),
        )?;
        let evaluated = evaluation(initial, OVERSCAN)?;
        let wanted = camera(
            &editor,
            viewport,
            1.5,
            Affine::new([1.5, 0., 0., 1.5, -292., -144.]),
        )?;
        let source_page =
            affine(evaluated.key.request)?.transform_rect_bbox(Rect::new(0., 0., 960., 720.));
        let plan = DisplayPlan::choose(evaluated, source_page, wanted)?;
        assert_eq!(plan.evaluation.key, evaluated.key);
        assert_eq!(plan.source_offset, [56, 58]);
        assert_eq!(plan.paint_size, [80, 60]);
        assert_eq!(plan.document_to_viewport.affine(), affine(wanted.request)?);
        assert_eq!(plan.mode, DisplayMode::RetainedAtNativeDensity);
        assert!(
            plan.pending,
            "A source crop cannot claim a fresh COVERAGE256 viewport evaluation."
        );
        Ok(())
    }
    #[test]
    fn required_density_or_fractional_translation_keeps_the_displayed_camera() -> AppResult<()> {
        let editor = Editor::demo()?;
        let viewport = Viewport {
            x: 20,
            y: 100,
            width: 80,
            height: 60,
        };
        let initial = camera(&editor, viewport, 1., Affine::translate((-300., -150.)))?;
        let evaluated = evaluation(initial, OVERSCAN)?;
        let page =
            affine(evaluated.key.request)?.transform_rect_bbox(Rect::new(0., 0., 960., 720.));
        for transform in [
            Affine::new([1.25, 0., 0., 1.25, -300., -150.]),
            Affine::translate((-299.5, -150.)),
            Affine::translate((-200., -150.)),
        ] {
            let wanted = camera(&editor, viewport, 1., transform)?;
            let plan = DisplayPlan::choose(evaluated, page, wanted)?;
            assert_eq!(plan.document_to_viewport.affine(), affine(initial.request)?);
            assert_eq!(plan.source_offset, [64, 64]);
            assert_eq!(plan.mode, DisplayMode::PendingCoverage);
            assert!(plan.pending);
        }
        Ok(())
    }
    #[test]
    fn fractional_dpi_and_snapped_origin_use_one_physical_mapping() -> AppResult<()> {
        let editor = Editor::demo()?;
        for scale in [1., 1.5, 2.] {
            let logical_origin = Point::new(46.25, 130.125);
            let viewport = Viewport {
                x: (logical_origin.x * scale).round() as u32,
                y: (logical_origin.y * scale).round() as u32,
                width: 120,
                height: 90,
            };
            let transform = Affine::new([scale, 0., 0., scale, 20.125, 12.75]);
            let initial = camera(&editor, viewport, scale, transform)?;
            let evaluated = evaluation(initial, OVERSCAN)?;
            let page =
                affine(evaluated.key.request)?.transform_rect_bbox(Rect::new(0., 0., 960., 720.));
            let plan = DisplayPlan::choose(evaluated, page, initial)?;
            let document = Point::new(24., 18.);
            let window = Point::new(
                (f64::from(viewport.x) + document.x * scale + 20.125) / scale,
                (f64::from(viewport.y) + document.y * scale + 12.75) / scale,
            );
            assert!(
                (plan
                    .document_point(window)
                    .ok_or("Mapped point was outside the viewport.")?
                    - document)
                    .hypot()
                    < 1e-10
            );
            let canvas = plan.logical_affine(logical_origin) * document;
            assert!((canvas + logical_origin.to_vec2() - window).hypot() < 1e-10);
            assert_eq!(plan.mode, DisplayMode::EvaluatedSource);
            assert!(!plan.pending);
        }
        Ok(())
    }
    #[test]
    fn zero_synthesis_requires_page_antialias_support_not_just_pixel_centers() {
        // Target pixel0 has center .5 before page start .75, but intersects page
        // support and cannot be synthesized when source coordinate -1 is absent.
        assert!(!covered(
            Rect::new(-0.25, 0., 3.75, 4.),
            [4, 4],
            [-1, 0],
            [4, 4]
        ));
        assert!(covered(Rect::new(0., 0., 4., 4.), [4, 4], [-1, 0], [4, 4]));
        assert!(!covered(
            Rect::new(0., 0., 4.25, 4.),
            [4, 4],
            [1, 0],
            [4, 4]
        ));
    }
    #[test]
    fn uncovered_resize_crops_without_stretching_or_claiming_new_density() -> AppResult<()> {
        let editor = Editor::demo()?;
        let original = Viewport {
            x: 20,
            y: 100,
            width: 80,
            height: 60,
        };
        let initial = camera(&editor, original, 1., Affine::translate((-300., -150.)))?;
        let evaluated = evaluation(initial, 0)?;
        let wanted = camera(
            &editor,
            Viewport {
                width: 160,
                height: 120,
                ..original
            },
            1.,
            Affine::translate((-260., -120.)),
        )?;
        let page =
            affine(evaluated.key.request)?.transform_rect_bbox(Rect::new(0., 0., 960., 720.));
        let plan = DisplayPlan::choose(evaluated, page, wanted)?;
        assert_eq!(plan.paint_size, [80, 60]);
        assert_eq!(plan.document_to_viewport.affine(), affine(initial.request)?);
        assert!(plan.document_point(Point::new(101., 101.)).is_none());
        assert!(plan.pending);
        Ok(())
    }
    #[test]
    fn evaluation_refuses_mislabeled_source_or_wrong_format() -> AppResult<()> {
        let editor = Editor::demo()?;
        let viewport = Viewport {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        };
        let camera = camera(&editor, viewport, 1., Affine::IDENTITY)?;
        let valid = evaluation(camera, OVERSCAN)?;
        assert!(
            Evaluation::new(
                Key {
                    request: camera.request,
                    ..valid.key
                },
                camera,
                OVERSCAN
            )
            .is_err()
        );
        let rgba8 = RenderRequest::new(editor.page()?, 8, 8, View::Affine(Transform::IDENTITY))?;
        assert!(CameraIntent::new(camera.revision, rgba8, viewport, 1.).is_err());
        assert!(CameraIntent::new(camera.revision, camera.request, viewport, f64::NAN).is_err());
        assert!(padded_request(camera.request, u32::MAX).is_err());
        Ok(())
    }
    #[test]
    fn narrow_prefetch_budget_keeps_the_full_visible_request() -> AppResult<()> {
        let editor = Editor::demo()?;
        let camera = camera(
            &editor,
            Viewport {
                x: 0,
                y: 0,
                width: 32,
                height: 24,
            },
            1.,
            Affine::IDENTITY,
        )?;
        let padded = padded_request(camera.request, OVERSCAN)?;
        let limits = affint_render::Limits {
            max_bytes: 256 * 1024,
            ..Default::default()
        };
        assert!(!prefetch_fits(padded, &limits, 16_384));
        assert!(prefetch_fits(camera.request, &limits, 16_384));
        assert!(!prefetch_fits(
            padded,
            &affint_render::Limits::default(),
            128
        ));
        let evaluated = evaluation(camera, 0)?;
        assert_eq!(
            (
                evaluated.key.request.width(),
                evaluated.key.request.height()
            ),
            (32, 24)
        );
        assert_eq!(evaluated.key.request.view(), camera.request.view());
        Ok(())
    }
    #[test]
    fn candidate_chrome_uses_candidate_metadata_and_current_receipt_survives_retry() -> AppResult<()>
    {
        let mut editor = Editor::demo()?;
        let viewport = Viewport {
            x: 0,
            y: 0,
            width: 32,
            height: 24,
        };
        let initial = camera(&editor, viewport, 1., Affine::IDENTITY)?;
        let current = evaluation(initial, OVERSCAN)?;
        let page = Rect::new(64., 64., 1024., 784.);
        let receipt = DisplayPlan::choose(current, page, initial)?;
        let newer = Evaluation::new(
            Key {
                serial: 12,
                ..current.key
            },
            initial,
            OVERSCAN,
        )?;
        let planned = select_plan(newer.key, page, Some(newer), Some(receipt), 7, initial)?
            .ok_or("Candidate plan is missing.")?;
        assert_eq!(planned.evaluation.key, newer.key);
        assert_eq!(
            receipt.evaluation.key, current.key,
            "Planning does not mutate the last actual presentation."
        );
        let held = select_plan(current.key, page, Some(newer), Some(receipt), 7, initial)?
            .ok_or("Current receipt metadata was lost while newer work was pending.")?;
        assert_eq!(held.evaluation.key, current.key);
        assert!(select_plan(newer.key, page, Some(current), Some(receipt), 7, initial).is_err());
        assert!(select_plan(newer.key, page, Some(newer), Some(receipt), 8, initial)?.is_none());
        editor.add_page()?;
        let another_page = camera(&editor, viewport, 1., Affine::IDENTITY)?;
        assert!(
            select_plan(newer.key, page, Some(newer), Some(receipt), 7, another_page)?.is_none()
        );
        Ok(())
    }
}
