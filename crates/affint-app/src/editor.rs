//! Application commands and transient selection, separate from document semantics.
use std::path::PathBuf;
use std::sync::Arc;

use affint_document::{
    Change, Color, Content, Edit, History, LayerSpec, Limits, LiveOperation, Mask, MaskMode,
    NodeId, PageId, PageSize, Parent, PixelSize, RasterAsset, Revision, Session, Snapshot,
    Transform, VectorContent,
};
use affint_render::{RenderRequest, View};
use kurbo::{Affine, BezPath, Ellipse, Point, Rect, Shape};

pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Move,
    Rectangle,
    Ellipse,
    Crop,
    Pan,
}

#[derive(Debug, Clone, Copy)]
enum Gesture {
    Move {
        node: NodeId,
        start: Point,
        initial: Affine,
        parent_inverse: Affine,
    },
    Shape {
        node: NodeId,
        start: Point,
        ellipse: bool,
    },
    Crop {
        node: NodeId,
        start: Point,
        inverse: Affine,
    },
    Opacity {
        node: NodeId,
    },
    Fill {
        node: Option<NodeId>,
        initial: [f64; 4],
    },
}

/// The only application owner of mutable artwork. Saves compare revision tokens.
pub struct Editor {
    pub session: Session,
    pub selected: Option<NodeId>,
    pub path: Option<PathBuf>,
    pub title: String,
    pub tool: Tool,
    pub fill: [f64; 4],
    saved: Option<Revision>,
    preview: Option<Snapshot>,
    gesture: Option<Gesture>,
    active_page: Option<PageId>,
    history_page: Option<PageId>,
}

pub fn document_limits() -> Limits {
    Limits {
        max_pages: 64,
        max_layers: 4096,
        max_assets: 512,
        max_name_bytes: 4096,
        max_pixel_bytes: 256 * 1024 * 1024,
        max_metadata_bytes: 96 * 1024 * 1024,
        max_history_metadata_bytes: 48 * 1024 * 1024,
        ..Limits::default()
    }
}

impl Editor {
    pub fn demo() -> AppResult<Self> {
        let session = sample_document()?;
        let active_page = session.snapshot().page_order().first().copied();
        let selected = session
            .snapshot()
            .layers()
            .find(|(_, node)| node.name() == "Coral disc")
            .map(|(id, _)| *id);
        Ok(Self {
            session,
            selected,
            path: None,
            title: "Chromatic study".into(),
            tool: Tool::Move,
            fill: srgb_fill([0.96, 0.36, 0.27, 1.0]),
            saved: None,
            preview: None,
            gesture: None,
            active_page,
            history_page: None,
        })
    }

    pub fn from_loaded(session: Session, path: PathBuf) -> Self {
        let active_page = session.snapshot().page_order().first().copied();
        let saved = Some(session.snapshot().revision());
        let title = path.file_stem().map_or_else(
            || "Untitled".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        Self {
            session,
            selected: None,
            path: Some(path),
            title,
            tool: Tool::Move,
            fill: srgb_fill([0.96, 0.36, 0.27, 1.0]),
            saved,
            preview: None,
            gesture: None,
            active_page,
            history_page: None,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.preview
            .clone()
            .unwrap_or_else(|| self.session.snapshot())
    }
    pub fn dirty(&self) -> bool {
        self.saved != Some(self.session.snapshot().revision()) || self.document_gesture()
    }
    fn document_gesture(&self) -> bool {
        self.gesture.is_some() && !matches!(self.gesture, Some(Gesture::Fill { node: None, .. }))
    }
    pub fn gesture_active(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn saved(&mut self, revision: Revision, path: PathBuf) {
        self.saved = Some(revision);
        self.title = path.file_stem().map_or_else(
            || "Untitled".into(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.path = Some(path);
    }
    pub fn page(&self) -> AppResult<PageId> {
        let page = self.active_page.ok_or("The document has no active page.")?;
        if self.snapshot().page(page).is_none() {
            return Err("The active page no longer exists.".into());
        }
        Ok(page)
    }
    pub fn switch_page(&mut self, page: PageId) -> AppResult<bool> {
        if self.session.snapshot().page(page).is_none() {
            return Err("This page no longer exists. Choose an existing page.".into());
        }
        self.cancel()?;
        let changed = self.active_page != Some(page);
        self.active_page = Some(page);
        self.history_page = None;
        if changed {
            self.selected = None;
        }
        Ok(changed)
    }
    pub fn adjacent_page(&self, forward: bool) -> AppResult<Option<PageId>> {
        let snapshot = self.snapshot();
        let active = self.page()?;
        let index = snapshot
            .page_order()
            .iter()
            .position(|page| *page == active)
            .ok_or("The active page order is missing.")?;
        let next = if forward {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        Ok(next.and_then(|index| snapshot.page_order().get(index).copied()))
    }
    /// Blank creation reuses validated dimensions, never artwork or pixel buffers.
    pub fn add_page(&mut self) -> AppResult<PageId> {
        self.cancel()?;
        let snapshot = self.session.snapshot();
        let size = snapshot
            .page(self.page()?)
            .ok_or("The page is missing.")?
            .size();
        let number = snapshot
            .page_order()
            .len()
            .checked_add(1)
            .ok_or("Page count overflow.")?;
        let created = self.session.transact(Edit::new(
            "Add blank page",
            [Change::AddPage {
                name: format!("Page {number}"),
                size,
            }],
        ))?;
        let page = created
            .pages
            .first()
            .copied()
            .ok_or("The new page identity is missing.")?;
        self.active_page = Some(page);
        self.history_page = None;
        self.selected = None;
        Ok(page)
    }
    /// Captures page and snapshot together before a queued dialog/export can run.
    pub fn export_request(&self) -> AppResult<(Snapshot, RenderRequest)> {
        let snapshot = self.session.snapshot();
        let page = self.page()?;
        let size = snapshot
            .page(page)
            .ok_or("The export page is missing.")?
            .size();
        if size.width().fract() != 0.
            || size.height().fract() != 0.
            || size.width() > f64::from(u32::MAX)
            || size.height() > f64::from(u32::MAX)
        {
            return Err("Exact PNG export requires whole-pixel page dimensions within the supported range. This page has unsupported dimensions.".into());
        }
        let request = RenderRequest::new(
            page,
            size.width() as u32,
            size.height() as u32,
            View::Region(Rect::new(0., 0., size.width(), size.height())),
        )?;
        Ok((snapshot, request))
    }
    pub fn selected_node(&self) -> AppResult<Option<NodeId>> {
        if let Some(node) = self.selected {
            self.require_active_layer(node)?;
        }
        Ok(self.selected)
    }
    fn require_active_layer(&self, node: NodeId) -> AppResult<()> {
        if node_page(&self.snapshot(), node)? != self.page()? {
            return Err(
                "This layer belongs to another page. Switch to its page before editing it.".into(),
            );
        }
        Ok(())
    }
    fn reconcile_pages(&mut self) -> AppResult<()> {
        let snapshot = self.session.snapshot();
        if let Some(page) = self
            .history_page
            .filter(|page| snapshot.page(*page).is_some())
        {
            self.active_page = Some(page);
            self.history_page = None;
        }
        if self
            .active_page
            .is_none_or(|page| snapshot.page(page).is_none())
        {
            self.history_page = self.active_page;
            self.active_page = snapshot.page_order().first().copied();
        }
        let page = self.page()?;
        if let Some(node) = self.selected
            && (snapshot.node(node).is_none() || node_page(&snapshot, node)? != page)
        {
            self.selected = None;
        }
        Ok(())
    }
    pub fn select(&mut self, node: NodeId) -> AppResult<()> {
        self.cancel()?;
        self.require_active_layer(node)?;
        self.selected = Some(node);
        Ok(())
    }
    pub fn transact(
        &mut self,
        label: &str,
        changes: impl IntoIterator<Item = Change>,
    ) -> AppResult<()> {
        self.session.transact(Edit::new(label, changes))?;
        self.reconcile_pages()?;
        Ok(())
    }
    pub fn history(&mut self, direction: History) -> AppResult<()> {
        self.cancel()?;
        self.session.history(direction)?;
        self.reconcile_pages()?;
        Ok(())
    }
    pub fn import(&mut self, page: PageId, asset: Arc<RasterAsset>, name: String) -> AppResult<()> {
        self.cancel()?;
        let snapshot = self.session.snapshot();
        let index = snapshot
            .page(page)
            .ok_or("The page is missing.")?
            .roots()
            .len();
        let created = self.session.transact(Edit::new(
            "Import PNG",
            [Change::InsertRaster {
                parent: Parent::Page(page),
                index,
                name,
                asset,
                transform: Transform::IDENTITY,
            }],
        ))?;
        if self.page()? == page {
            self.selected = created.nodes.first().copied();
        }
        Ok(())
    }
    pub fn remove(&mut self) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        self.cancel()?;
        let snapshot = self.session.snapshot();
        let mut changes: Vec<Change> = snapshot
            .layers()
            .filter(|(_, layer)| layer.mask().is_some_and(|mask| mask.source == node))
            .map(|(id, _)| Change::SetMask {
                node: *id,
                mask: None,
            })
            .collect();
        changes.push(Change::RemoveLayer { node });
        self.transact("Remove layer", changes)?;
        self.selected = None;
        Ok(())
    }
    pub fn reorder(&mut self, forward: bool) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        let snapshot = self.session.snapshot();
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        let parent = layer.parent();
        let siblings = match parent {
            Parent::Page(id) => snapshot.page(id).ok_or("The page is missing.")?.roots(),
            Parent::Layer(id) => snapshot.node(id).ok_or("The group is missing.")?.children(),
        };
        let index = siblings
            .iter()
            .position(|id| *id == node)
            .ok_or("The layer order is inconsistent.")?;
        let next = if forward {
            (index + 1).min(siblings.len().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        if next != index {
            self.transact(
                "Reorder layer",
                [Change::MoveLayer {
                    node,
                    parent,
                    index: next,
                }],
            )?;
        }
        Ok(())
    }
    pub fn set_fill(&mut self, srgb: [f64; 4]) -> AppResult<()> {
        let fill = validated_srgb_fill(srgb)?;
        if let Some(node) = self.selected_node()? {
            let snapshot = self.session.snapshot();
            let layer = snapshot.node(node).ok_or("The layer is missing.")?;
            if let Content::Vector(vector) = layer.content()
                && vector.fill() != Color::try_from(fill)?
            {
                let content = Content::Vector(vector.with_fill(Color::try_from(fill)?));
                let spec = layer_spec_with_content(layer, content);
                self.transact("Change fill", [Change::SetLayer { node, layer: spec }])?;
            }
        }
        self.fill = fill;
        Ok(())
    }
    /// A selected vector previews one document edit; other selections change only
    /// the foreground for future shapes, without recoloring original raster pixels.
    pub fn begin_fill(&mut self) -> AppResult<()> {
        self.cancel()?;
        let snapshot = self.snapshot();
        let node = self.selected_node()?.filter(|node| {
            snapshot
                .node(*node)
                .is_some_and(|layer| matches!(layer.content(), Content::Vector(_)))
        });
        if node.is_some() {
            self.session.begin_gesture("Change fill")?;
        }
        self.gesture = Some(Gesture::Fill {
            node,
            initial: self.fill,
        });
        Ok(())
    }
    /// Encoded sRGB RGBA in [0,1], validated before any preview/default changes.
    pub fn fill(&mut self, srgb: [f64; 4]) -> AppResult<()> {
        let fill = validated_srgb_fill(srgb)?;
        let Some(Gesture::Fill { node, .. }) = self.gesture else {
            return Ok(());
        };
        if let Some(node) = node {
            let snapshot = self.snapshot();
            let layer = snapshot.node(node).ok_or("The fill layer is missing.")?;
            let Content::Vector(vector) = layer.content() else {
                return Err("The selected fill is no longer editable vector content.".into());
            };
            let color = Color::try_from(fill)?;
            if vector.fill() != color {
                let content = Content::Vector(vector.with_fill(color));
                let spec = layer_spec_with_content(layer, content);
                let (preview, _) = self.session.update_gesture(Edit::new(
                    "Preview fill",
                    [Change::SetLayer { node, layer: spec }],
                ))?;
                self.preview = Some(preview);
            }
        }
        self.fill = fill;
        Ok(())
    }
    pub fn exposure(&mut self, delta: f64) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        let snapshot = self.session.snapshot();
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        let mut ops = layer.operations().to_vec();
        if let Some(LiveOperation::Exposure(stops)) = ops
            .iter_mut()
            .find(|op| matches!(op, LiveOperation::Exposure(_)))
        {
            *stops = (*stops + delta).clamp(-8., 8.);
        } else {
            ops.push(LiveOperation::Exposure(delta));
        }
        self.transact(
            "Change exposure",
            [Change::SetOperations {
                node,
                operations: ops,
            }],
        )
    }
    pub fn clear_crop(&mut self) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        let snapshot = self.session.snapshot();
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        let ops: Vec<_> = layer
            .operations()
            .iter()
            .filter(|op| !matches!(op, LiveOperation::Crop(_)))
            .cloned()
            .collect();
        if ops.len() != layer.operations().len() {
            self.transact(
                "Remove crop",
                [Change::SetOperations {
                    node,
                    operations: ops,
                }],
            )?;
        }
        Ok(())
    }
    pub fn toggle_mask(&mut self) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        let snapshot = self.session.snapshot();
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        if layer.mask().is_some() {
            return self.transact("Disable mask", [Change::SetMask { node, mask: None }]);
        }
        let bounds = local_bounds(&snapshot, node)
            .ok_or("Select a raster or vector layer to add a mask.")?;
        let vector = VectorContent::new(
            Ellipse::from_rect(bounds.inset(-bounds.width().min(bounds.height()) * 0.04))
                .to_path(0.05),
            Color::new(1., 1., 1., 1.)?,
        )?;
        let mut spec = LayerSpec::new("Ellipse mask", Content::Vector(vector));
        spec.visible = false;
        // Creation and reference are one gesture because the new identity is needed
        // for the second command. Neither step reaches committed artwork alone.
        self.session.begin_gesture("Add editable mask")?;
        let outcome = (|| -> AppResult<()> {
            let page = self.page()?;
            let index = snapshot
                .page(page)
                .ok_or("The page is missing.")?
                .roots()
                .len();
            let (preview, created) = self.session.update_gesture(Edit::new(
                "Create mask",
                [Change::InsertLayer {
                    parent: Parent::Page(page),
                    index,
                    layer: spec,
                }],
            ))?;
            let source = created
                .nodes
                .first()
                .copied()
                .ok_or("The mask identity was not created.")?;
            let _ = preview;
            self.session.update_gesture(Edit::new(
                "Assign mask",
                [Change::SetMask {
                    node,
                    mask: Some(Mask {
                        source,
                        mode: MaskMode::Alpha,
                        inverted: false,
                    }),
                }],
            ))?;
            self.session.commit_gesture()?;
            Ok(())
        })();
        if outcome.is_err() {
            self.session.cancel_gesture()?;
        }
        outcome
    }
    pub fn toggle_visible(&mut self, node: NodeId) -> AppResult<()> {
        self.require_active_layer(node)?;
        let snapshot = self.session.snapshot();
        let mut spec = layer_spec(snapshot.node(node).ok_or("The layer is missing.")?);
        spec.visible = !spec.visible;
        self.transact(
            "Change visibility",
            [Change::SetLayer { node, layer: spec }],
        )
    }
    pub fn begin_opacity(&mut self) -> AppResult<()> {
        let Some(node) = self.selected_node()? else {
            return Ok(());
        };
        self.cancel()?;
        self.session.begin_gesture("Change opacity")?;
        self.gesture = Some(Gesture::Opacity { node });
        Ok(())
    }
    pub fn opacity(&mut self, value: f64) -> AppResult<()> {
        let Some(Gesture::Opacity { node }) = self.gesture else {
            return Ok(());
        };
        let snapshot = self.snapshot();
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        let opacity = value.clamp(0., 1.);
        if layer.opacity() == opacity {
            return Ok(());
        }
        let mut spec = layer_spec(layer);
        spec.opacity = opacity;
        let (snapshot, _) = self.session.update_gesture(Edit::new(
            "Change opacity",
            [Change::SetLayer { node, layer: spec }],
        ))?;
        self.preview = Some(snapshot);
        Ok(())
    }
    pub fn press(&mut self, at: Point) -> AppResult<()> {
        self.cancel()?;
        match self.tool {
            Tool::Pan => {}
            Tool::Move => {
                let snapshot = self.session.snapshot();
                self.selected = hit_layer(&snapshot, self.page()?, at)?;
                if let Some(node) = self.selected {
                    let layer = snapshot.node(node).ok_or("The layer is missing.")?;
                    let initial = layer.transform().affine();
                    let parent_inverse = match layer.parent() {
                        Parent::Page(_) => Affine::IDENTITY,
                        Parent::Layer(parent) => world_transform(&snapshot, parent)?.inverse(),
                    };
                    self.session.begin_gesture("Move layer")?;
                    self.gesture = Some(Gesture::Move {
                        node,
                        start: at,
                        initial,
                        parent_inverse,
                    });
                }
            }
            Tool::Rectangle | Tool::Ellipse => {
                let page = self.page()?;
                let index = self
                    .session
                    .snapshot()
                    .page(page)
                    .ok_or("The page is missing.")?
                    .roots()
                    .len();
                let ellipse = self.tool == Tool::Ellipse;
                let vector = shape(
                    Rect::new(at.x, at.y, at.x + 0.01, at.y + 0.01),
                    ellipse,
                    self.fill,
                )?;
                self.session.begin_gesture(if ellipse {
                    "Draw ellipse"
                } else {
                    "Draw rectangle"
                })?;
                match self.session.update_gesture(Edit::new(
                    "Start shape",
                    [Change::InsertLayer {
                        parent: Parent::Page(page),
                        index,
                        layer: LayerSpec::new(
                            if ellipse { "Ellipse" } else { "Rectangle" },
                            Content::Vector(vector),
                        ),
                    }],
                )) {
                    Ok((snapshot, created)) => {
                        let node = created
                            .nodes
                            .first()
                            .copied()
                            .ok_or("The shape identity was not created.")?;
                        self.selected = Some(node);
                        self.preview = Some(snapshot);
                        self.gesture = Some(Gesture::Shape {
                            node,
                            start: at,
                            ellipse,
                        });
                    }
                    Err(error) => {
                        self.session.cancel_gesture()?;
                        return Err(error.into());
                    }
                }
            }
            Tool::Crop => {
                if let Some(node) = self.selected_node()? {
                    let inverse = world_transform(&self.session.snapshot(), node)?.inverse();
                    self.session.begin_gesture("Crop layer")?;
                    self.gesture = Some(Gesture::Crop {
                        node,
                        start: inverse * at,
                        inverse,
                    });
                }
            }
        }
        Ok(())
    }
    pub fn drag(&mut self, at: Point) -> AppResult<()> {
        let Some(gesture) = self.gesture else {
            return Ok(());
        };
        // A held drag remains active during paint/replay frames with no motion.
        // Publish only a semantic change; a fresh preview would cancel its render.
        let preview = self.snapshot();
        let change = match gesture {
            Gesture::Move {
                node,
                start,
                initial,
                parent_inverse,
            } => {
                let delta = parent_inverse * at - parent_inverse * start;
                let transform = Transform::new((Affine::translate(delta) * initial).as_coeffs())?;
                if preview
                    .node(node)
                    .ok_or("The layer is missing.")?
                    .transform()
                    == transform
                {
                    return Ok(());
                }
                Change::SetTransform { node, transform }
            }
            Gesture::Shape {
                node,
                start,
                ellipse,
            } => {
                let bounds = drag_rect(start, at);
                let layer = preview.node(node).ok_or("The shape is missing.")?;
                let vector = shape(bounds, ellipse, self.fill)?;
                if let Content::Vector(current) = layer.content()
                    && current.path().elements() == vector.path().elements()
                    && current.fill() == vector.fill()
                {
                    return Ok(());
                }
                let mut spec = layer_spec(layer);
                spec.content = Content::Vector(vector);
                Change::SetLayer { node, layer: spec }
            }
            Gesture::Crop {
                node,
                start,
                inverse,
            } => {
                let layer = preview.node(node).ok_or("The layer is missing.")?;
                let mut ops: Vec<_> = layer
                    .operations()
                    .iter()
                    .filter(|op| !matches!(op, LiveOperation::Crop(_)))
                    .cloned()
                    .collect();
                ops.push(LiveOperation::Crop(drag_rect(start, inverse * at)));
                if layer.operations() == ops.as_slice() {
                    return Ok(());
                }
                Change::SetOperations {
                    node,
                    operations: ops,
                }
            }
            Gesture::Opacity { .. } | Gesture::Fill { .. } => return Ok(()),
        };
        let (snapshot, _) = self
            .session
            .update_gesture(Edit::new("Preview gesture", [change]))?;
        self.preview = Some(snapshot);
        Ok(())
    }
    pub fn commit(&mut self) -> AppResult<()> {
        if self.document_gesture() {
            self.session.commit_gesture()?;
        }
        self.gesture = None;
        self.preview = None;
        Ok(())
    }
    pub fn cancel(&mut self) -> AppResult<()> {
        if self.document_gesture() {
            self.session.cancel_gesture()?;
        }
        if let Some(Gesture::Fill { initial, .. }) = self.gesture {
            self.fill = initial;
        }
        self.gesture = None;
        self.preview = None;
        self.reconcile_pages()?;
        Ok(())
    }
}

fn shape(bounds: Rect, ellipse: bool, fill: [f64; 4]) -> AppResult<VectorContent> {
    let path = if ellipse {
        Ellipse::from_rect(bounds).to_path(0.05)
    } else {
        bounds.to_path(0.05)
    };
    Ok(VectorContent::new(path, Color::try_from(fill)?)?)
}
fn drag_rect(a: Point, b: Point) -> Rect {
    Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        a.x.max(b.x).max(a.x.min(b.x) + 0.01),
        a.y.max(b.y).max(a.y.min(b.y) + 0.01),
    )
}
pub fn layer_spec(layer: &affint_document::Layer) -> LayerSpec {
    layer_spec_with_content(layer, layer.content().clone())
}
fn layer_spec_with_content(layer: &affint_document::Layer, content: Content) -> LayerSpec {
    LayerSpec {
        name: layer.name().into(),
        content,
        transform: layer.transform(),
        opacity: layer.opacity(),
        visible: layer.visible(),
        mask: layer.mask(),
        operations: layer.operations().to_vec(),
    }
}
pub fn srgb_fill([r, g, b, a]: [f64; 4]) -> [f64; 4] {
    [decode_srgb(r), decode_srgb(g), decode_srgb(b), a]
}
fn validated_srgb_fill(srgb: [f64; 4]) -> AppResult<[f64; 4]> {
    if srgb
        .iter()
        .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
    {
        return Err("The colour control requires finite sRGB RGBA values from0 to1.".into());
    }
    Ok(srgb_fill(srgb))
}
pub fn decode_srgb(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
pub fn local_bounds(snapshot: &Snapshot, node: NodeId) -> Option<Rect> {
    match snapshot.node(node)?.content() {
        Content::Raster { asset } => {
            let size = snapshot.asset(*asset)?.size();
            Some(Rect::new(
                0.,
                0.,
                f64::from(size.width()),
                f64::from(size.height()),
            ))
        }
        Content::Vector(vector) => Some(vector.path().bounding_box()),
        Content::Group => None,
    }
}
pub fn world_transform(snapshot: &Snapshot, node: NodeId) -> AppResult<Affine> {
    let mut current = node;
    let mut result = Affine::IDENTITY;
    for _ in 0..256 {
        let layer = snapshot.node(current).ok_or("The layer is missing.")?;
        result = layer.transform().affine() * result;
        match layer.parent() {
            Parent::Page(_) => return Ok(result),
            Parent::Layer(parent) => current = parent,
        }
    }
    Err("The layer hierarchy is too deep.".into())
}
fn node_page(snapshot: &Snapshot, node: NodeId) -> AppResult<PageId> {
    let mut current = node;
    for _ in 0..256 {
        match snapshot
            .node(current)
            .ok_or("The layer is missing.")?
            .parent()
        {
            Parent::Page(page) => {
                if snapshot.page(page).is_none() {
                    return Err("The layer's page is missing.".into());
                }
                return Ok(page);
            }
            Parent::Layer(parent) => current = parent,
        }
    }
    Err("The layer hierarchy is too deep.".into())
}
pub fn selection_bounds(snapshot: &Snapshot, node: NodeId) -> AppResult<Option<Rect>> {
    if snapshot.node(node).is_none() {
        return Err("The selected layer no longer exists.".into());
    }
    local_bounds(snapshot, node)
        .map(|bounds| {
            world_transform(snapshot, node).map(|transform| transform.transform_rect_bbox(bounds))
        })
        .transpose()
}
fn hit_layer(snapshot: &Snapshot, page: PageId, at: Point) -> AppResult<Option<NodeId>> {
    let mut stack: Vec<NodeId> = snapshot
        .page(page)
        .ok_or("The page is missing.")?
        .roots()
        .to_vec();
    while let Some(node) = stack.pop() {
        let layer = snapshot.node(node).ok_or("The layer is missing.")?;
        if !layer.visible() {
            continue;
        }
        if matches!(layer.content(), Content::Group) {
            stack.extend(layer.children());
            continue;
        }
        let local = world_transform(snapshot, node)?.inverse() * at;
        let Some(bounds) = local_bounds(snapshot, node) else {
            continue;
        };
        if !bounds.contains(local) {
            continue;
        }
        if layer
            .operations()
            .iter()
            .any(|op| matches!(op, LiveOperation::Crop(rect) if !rect.contains(local)))
        {
            continue;
        }
        let contains = match layer.content() {
            Content::Vector(vector) => vector.path().winding(local) != 0,
            Content::Raster { .. } => true,
            Content::Group => false,
        };
        if contains {
            return Ok(Some(node));
        }
    }
    Ok(None)
}

/// Original synthetic vector/raster composition; source pixels never encode UI.
pub fn sample_document() -> AppResult<Session> {
    let mut session = Session::new(PageSize::new(960., 720.)?, document_limits())?;
    let page = session.snapshot().page_order()[0];
    let size = PixelSize::new(480, 360)?;
    let mut pixels = Vec::with_capacity(size.byte_len()?);
    for y in 0..size.height() {
        for x in 0..size.width() {
            let u = f64::from(x) / f64::from(size.width());
            let v = f64::from(y) / f64::from(size.height());
            let wave = ((u * 13. + v * 8.).sin() * 0.5 + 0.5) * 0.2;
            pixels.extend_from_slice(&[
                ((0.08 + 0.14 * v + wave) * 255.) as u8,
                ((0.16 + 0.24 * u + wave) * 255.) as u8,
                ((0.24 + 0.31 * (1. - v) + wave) * 255.) as u8,
                255,
            ]);
        }
    }
    let asset = Arc::new(RasterAsset::new(size, pixels)?);
    let mut background = LayerSpec::new(
        "Warm paper",
        Content::Vector(shape(
            Rect::new(0., 0., 960., 720.),
            false,
            srgb_fill([0.91, 0.88, 0.79, 1.]),
        )?),
    );
    background.transform = Transform::IDENTITY;
    session.transact(Edit::new(
        "Create original study",
        [
            Change::InsertLayer {
                parent: Parent::Page(page),
                index: 0,
                layer: background,
            },
            Change::InsertRaster {
                parent: Parent::Page(page),
                index: 1,
                name: "Tidal field · original 480 × 360".into(),
                asset,
                transform: Transform::new([1.6, 0., 0., 1.6, 96., 72.])?,
            },
            Change::InsertLayer {
                parent: Parent::Page(page),
                index: 2,
                layer: LayerSpec::new(
                    "Coral disc",
                    Content::Vector(shape(
                        Rect::new(550., 152., 778., 380.),
                        true,
                        srgb_fill([0.96, 0.36, 0.27, 1.]),
                    )?),
                ),
            },
            Change::InsertLayer {
                parent: Parent::Page(page),
                index: 3,
                layer: LayerSpec::new(
                    "Cobalt column",
                    Content::Vector(shape(
                        Rect::new(228., 236., 330., 624.),
                        false,
                        srgb_fill([0.13, 0.28, 0.93, 1.]),
                    )?),
                ),
            },
        ],
    ))?;
    let mut ribbon = BezPath::new();
    ribbon.move_to((118., 474.));
    ribbon.curve_to((338., 328.), (574., 594.), (840., 422.));
    ribbon.line_to((840., 484.));
    ribbon.curve_to((566., 662.), (352., 416.), (118., 534.));
    ribbon.close_path();
    let mut spec = LayerSpec::new(
        "Ivory current",
        Content::Vector(VectorContent::new(
            ribbon,
            Color::try_from(srgb_fill([0.96, 0.93, 0.82, 1.]))?,
        )?),
    );
    spec.opacity = 0.88;
    session.transact(Edit::new(
        "Draw current",
        [Change::InsertLayer {
            parent: Parent::Page(page),
            index: 4,
            layer: spec,
        }],
    ))?;
    // Initial content is a document, not an undoable boot sequence.
    Session::from_archive(session.snapshot().archive()?, document_limits()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn two_raster_pages() -> AppResult<(Editor, PageId, NodeId, PageId, NodeId)> {
        let session = Session::new(PageSize::new(16., 12.)?, document_limits())?;
        let mut editor = Editor::from_loaded(session, PathBuf::from("two-pages.affint"));
        let first = editor.page()?;
        let size = PixelSize::new(4, 4)?;
        editor.import(
            first,
            Arc::new(RasterAsset::new(size, [245, 45, 35, 255].repeat(16))?),
            "Red original".into(),
        )?;
        let red = editor.selected_node()?.ok_or("No red layer.")?;
        let second = editor.add_page()?;
        editor.import(
            second,
            Arc::new(RasterAsset::new(size, [35, 95, 225, 255].repeat(16))?),
            "Blue original".into(),
        )?;
        let blue = editor.selected_node()?.ok_or("No blue layer.")?;
        Ok((editor, first, red, second, blue))
    }
    #[test]
    fn page_scoped_selection_move_and_import_preserve_other_artwork() -> AppResult<()> {
        let (mut editor, first, red, second, blue) = two_raster_pages()?;
        let original = editor.session.snapshot();
        editor.switch_page(first)?;
        assert!(editor.select(blue).is_err());
        editor.press(Point::new(1., 1.))?;
        assert_eq!(editor.selected_node()?, Some(red));
        editor.drag(Point::new(6., 4.))?;
        editor.commit()?;
        let moved = editor.session.snapshot();
        assert_ne!(
            moved.node(red).ok_or("No red layer.")?.transform(),
            original.node(red).ok_or("No red original.")?.transform()
        );
        assert_eq!(
            moved.node(blue).ok_or("No blue layer.")?.transform(),
            original.node(blue).ok_or("No blue original.")?.transform()
        );
        for (id, asset) in original.assets() {
            assert_eq!(
                moved.asset(*id).ok_or("Missing source.")?.pixels(),
                asset.pixels()
            );
        }
        editor.switch_page(second)?;
        editor.press(Point::new(1., 1.))?;
        assert_eq!(editor.selected_node()?, Some(blue));
        editor.drag(Point::new(3., 3.))?;
        editor.commit()?;
        assert_eq!(
            editor
                .session
                .snapshot()
                .node(red)
                .ok_or("No red layer.")?
                .transform(),
            moved.node(red).ok_or("No moved red layer.")?.transform()
        );
        let blue_transform = editor
            .session
            .snapshot()
            .node(blue)
            .ok_or("No blue layer.")?
            .transform();
        editor.import(
            first,
            Arc::new(RasterAsset::new(
                PixelSize::new(1, 1)?,
                vec![60, 180, 90, 255],
            )?),
            "Captured first-page import".into(),
        )?;
        assert_eq!(editor.page()?, second);
        assert_eq!(editor.selected_node()?, Some(blue));
        assert_eq!(
            editor
                .session
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .transform(),
            blue_transform
        );
        assert_eq!(
            editor
                .session
                .snapshot()
                .page(first)
                .ok_or("No first page.")?
                .roots()
                .len(),
            2
        );
        assert_eq!(
            editor
                .session
                .snapshot()
                .page(second)
                .ok_or("No second page.")?
                .roots()
                .len(),
            1
        );
        Ok(())
    }
    #[test]
    fn blank_page_history_restores_valid_identity_without_copying_sources() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let first = editor.page()?;
        let original = editor.session.snapshot();
        let page = editor.add_page()?;
        let added = editor.session.snapshot();
        assert_eq!(editor.page()?, page);
        assert!(editor.selected_node()?.is_none());
        assert!(added.page(page).ok_or("No new page.")?.roots().is_empty());
        assert_eq!(
            added.page(page).ok_or("No new page.")?.size(),
            original.page(first).ok_or("No first page.")?.size()
        );
        assert_eq!(added.assets().count(), original.assets().count());
        for (id, asset) in original.assets() {
            assert_eq!(
                added
                    .asset(*id)
                    .ok_or("Missing shared source.")?
                    .pixels()
                    .as_ptr(),
                asset.pixels().as_ptr()
            );
        }
        assert_eq!(editor.session.history_status().undo, 1);
        editor.history(History::Undo)?;
        assert_eq!(editor.page()?, first);
        assert!(editor.session.snapshot().page(page).is_none());
        assert!(editor.switch_page(page).is_err());
        editor.history(History::Redo)?;
        assert_eq!(editor.page()?, page);
        assert!(editor.selected_node()?.is_none());
        editor.history(History::Undo)?;
        let branch = editor.add_page()?;
        assert_ne!(branch, page);
        assert_eq!(editor.session.history_status().redo, 0);
        for (id, asset) in original.assets() {
            assert_eq!(
                editor
                    .session
                    .snapshot()
                    .asset(*id)
                    .ok_or("Missing original.")?
                    .pixels(),
                asset.pixels()
            );
        }
        Ok(())
    }
    #[test]
    fn explicit_page_choice_survives_redo_and_limit_failure_is_atomic() -> AppResult<()> {
        let mut editor = Editor::from_loaded(
            Session::new(
                PageSize::new(16., 12.)?,
                Limits {
                    max_pages: 3,
                    ..document_limits()
                },
            )?,
            PathBuf::from("bounded-pages.affint"),
        );
        let first = editor.page()?;
        let second = editor.add_page()?;
        let third = editor.add_page()?;
        editor.history(History::Undo)?;
        assert_eq!(editor.page()?, first);
        editor.switch_page(second)?;
        editor.history(History::Redo)?;
        assert_eq!(editor.page()?, second);
        assert!(editor.session.snapshot().page(third).is_some());
        let before = editor.session.snapshot();
        let undo = editor.session.history_status().undo;
        assert!(editor.add_page().is_err());
        assert_eq!(editor.page()?, second);
        assert_eq!(editor.session.snapshot().revision(), before.revision());
        assert_eq!(editor.session.history_status().undo, undo);
        assert_eq!(editor.session.snapshot().page_order(), before.page_order());
        Ok(())
    }
    #[test]
    fn page_switch_cancels_gesture_without_mutating_document_or_dirty_state() -> AppResult<()> {
        let (mut editor, first, red, second, _) = two_raster_pages()?;
        editor.switch_page(first)?;
        let before = editor.session.snapshot();
        let undo = editor.session.history_status().undo;
        editor.saved(before.revision(), PathBuf::from("two-pages.affint"));
        editor.press(Point::new(1., 1.))?;
        editor.drag(Point::new(8., 7.))?;
        assert!(editor.gesture_active());
        editor.switch_page(second)?;
        assert!(!editor.gesture_active());
        assert!(!editor.dirty());
        assert!(editor.selected_node()?.is_none());
        assert_eq!(editor.session.snapshot().revision(), before.revision());
        assert_eq!(editor.session.history_status().undo, undo);
        assert_eq!(
            editor
                .session
                .snapshot()
                .node(red)
                .ok_or("No red layer.")?
                .transform(),
            before.node(red).ok_or("No red original.")?.transform()
        );
        editor.tool = Tool::Rectangle;
        editor.press(Point::new(7., 6.))?;
        editor.drag(Point::new(10., 9.))?;
        editor.switch_page(first)?;
        assert!(editor.selected_node()?.is_none());
        assert_eq!(editor.session.snapshot().revision(), before.revision());
        Ok(())
    }
    #[test]
    fn captured_active_page_export_is_unchanged_by_later_navigation() -> AppResult<()> {
        let (mut editor, first, _, second, _) = two_raster_pages()?;
        let (snapshot, request) = editor.export_request()?;
        assert_eq!(request.page(), second);
        editor.switch_page(first)?;
        let (other_snapshot, other_request) = editor.export_request()?;
        assert_eq!(snapshot.revision(), other_snapshot.revision());
        assert_eq!(other_request.page(), first);
        let renderer = affint_render::Renderer::new(Default::default())?;
        let captured = renderer.render(&snapshot, request)?;
        let other = renderer.render(&other_snapshot, other_request)?;
        assert_eq!(
            &captured
                .rgba8()
                .ok_or("Captured export did not produce RGBA8.")?[0..4],
            &[35, 95, 225, 255]
        );
        assert_eq!(
            &other.rgba8().ok_or("Other export did not produce RGBA8.")?[0..4],
            &[245, 45, 35, 255]
        );
        assert_ne!(
            captured
                .rgba8()
                .ok_or("Captured export did not produce RGBA8.")?,
            other.rgba8().ok_or("Other export did not produce RGBA8.")?
        );
        Ok(())
    }
    #[test]
    fn older_save_receipt_cannot_clear_new_edits() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let saved = editor.session.snapshot().revision();
        editor.exposure(0.25)?;
        editor.saved(saved, PathBuf::from("study.affint"));
        assert!(editor.dirty());
        Ok(())
    }
    #[test]
    fn canceled_move_preserves_artwork_and_history() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let before = editor.session.snapshot();
        editor.press(Point::new(660., 260.))?;
        editor.drag(Point::new(710., 290.))?;
        editor.cancel()?;
        assert_eq!(editor.session.snapshot().revision(), before.revision());
        assert_eq!(editor.session.history_status().undo, 0);
        Ok(())
    }
    #[test]
    fn continuous_move_is_one_undo_and_preserves_source_pixels() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let before = editor.session.snapshot();
        let (asset_id, source) = before.assets().next().ok_or("No original source.")?;
        editor.press(Point::new(660., 260.))?;
        editor.drag(Point::new(680., 270.))?;
        editor.drag(Point::new(700., 290.))?;
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, 1);
        editor.history(History::Undo)?;
        assert_eq!(
            editor
                .session
                .snapshot()
                .asset(*asset_id)
                .ok_or("No original source.")?
                .pixels(),
            source.pixels()
        );
        Ok(())
    }

    #[test]
    fn stationary_move_keeps_revision_and_changed_move_commits_one_edit() -> AppResult<()> {
        let (mut editor, _, _, _, blue) = two_raster_pages()?;
        let original = editor.session.snapshot();
        let undo = editor.session.history_status().undo;
        editor.saved(original.revision(), PathBuf::from("stationary.affint"));
        editor.press(Point::new(1., 1.))?;
        for _ in 0..4 {
            editor.drag(Point::new(1., 1.))?;
            assert_eq!(editor.snapshot().revision(), original.revision());
        }
        editor.commit()?;
        assert_eq!(editor.session.snapshot().revision(), original.revision());
        assert_eq!(editor.session.history_status().undo, undo);
        assert!(!editor.dirty());

        editor.press(Point::new(1., 1.))?;
        editor.drag(Point::new(2., 3.))?;
        let preview = editor.snapshot();
        assert_ne!(preview.revision(), original.revision());
        for _ in 0..4 {
            editor.drag(Point::new(2., 3.))?;
            assert_eq!(editor.snapshot().revision(), preview.revision());
            assert_eq!(editor.session.snapshot().revision(), original.revision());
            assert_eq!(editor.session.history_status().undo, undo);
        }
        editor.drag(Point::new(4., 5.))?;
        assert_ne!(editor.snapshot().revision(), preview.revision());
        let moved = Transform::new([1., 0., 0., 1., 3., 4.])?;
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, undo + 1);
        assert_eq!(
            editor
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .transform(),
            moved
        );
        editor.history(History::Undo)?;
        assert_eq!(
            editor
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .transform(),
            Transform::IDENTITY
        );
        editor.history(History::Redo)?;
        assert_eq!(
            editor
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .transform(),
            moved
        );
        for (id, source) in original.assets() {
            let current = editor.snapshot();
            let retained = current.asset(*id).ok_or("No original source.")?;
            assert_eq!(retained.pixels(), source.pixels());
            assert_eq!(retained.pixels().as_ptr(), source.pixels().as_ptr());
        }
        Ok(())
    }

    #[test]
    fn stationary_shape_and_crop_previews_do_not_churn_and_cancel_preserves_artwork()
    -> AppResult<()> {
        for tool in [Tool::Rectangle, Tool::Ellipse, Tool::Crop] {
            let (mut editor, _, _, _, _) = two_raster_pages()?;
            let original = editor.session.snapshot();
            let undo = editor.session.history_status().undo;
            editor.tool = tool;
            editor.press(Point::new(6., 5.))?;
            editor.drag(Point::new(10., 9.))?;
            let preview = editor.snapshot().revision();
            for _ in 0..4 {
                editor.drag(Point::new(10., 9.))?;
                assert_eq!(editor.snapshot().revision(), preview, "{tool:?}");
                assert_eq!(editor.session.history_status().undo, undo, "{tool:?}");
            }
            editor.drag(Point::new(11., 10.))?;
            assert_ne!(editor.snapshot().revision(), preview, "{tool:?}");
            editor.cancel()?;
            let restored = editor.snapshot();
            assert_eq!(restored.revision(), original.revision(), "{tool:?}");
            assert_eq!(
                restored.layers().count(),
                original.layers().count(),
                "{tool:?}"
            );
            assert_eq!(editor.session.history_status().undo, undo, "{tool:?}");
            for (id, source) in original.assets() {
                let retained = restored.asset(*id).ok_or("No original source.")?;
                assert_eq!(retained.pixels(), source.pixels());
                assert_eq!(retained.pixels().as_ptr(), source.pixels().as_ptr());
            }
        }
        Ok(())
    }

    #[test]
    fn repeated_opacity_and_clamped_values_keep_preview_revision_and_one_undo() -> AppResult<()> {
        let (mut editor, _, _, _, blue) = two_raster_pages()?;
        let original = editor.snapshot();
        let undo = editor.session.history_status().undo;
        editor.begin_opacity()?;
        for value in [1., 2., 1.] {
            editor.opacity(value)?;
            assert_eq!(editor.snapshot().revision(), original.revision());
        }
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, undo);
        assert_eq!(editor.snapshot().revision(), original.revision());
        editor.begin_opacity()?;
        editor.opacity(0.6)?;
        let preview = editor.snapshot().revision();
        for _ in 0..4 {
            editor.opacity(0.6)?;
            assert_eq!(editor.snapshot().revision(), preview);
        }
        editor.opacity(0.4)?;
        assert_ne!(editor.snapshot().revision(), preview);
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, undo + 1);
        assert_eq!(
            editor
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .opacity(),
            0.4
        );
        editor.history(History::Undo)?;
        assert_eq!(
            editor
                .snapshot()
                .node(blue)
                .ok_or("No blue layer.")?
                .opacity(),
            1.
        );
        let restored = editor.snapshot().revision();
        editor.begin_opacity()?;
        editor.opacity(0.3)?;
        editor.opacity(0.3)?;
        editor.cancel()?;
        assert_eq!(editor.snapshot().revision(), restored);
        assert_eq!(editor.session.history_status().undo, undo);
        Ok(())
    }

    #[test]
    fn fill_preview_is_atomic_noop_stable_and_reversible_with_original_sources() -> AppResult<()> {
        let mut editor = Editor::demo()?;
        let node = editor.selected.ok_or("No selected vector.")?;
        let original = editor.snapshot();
        let original_content = original.node(node).ok_or("No vector.")?.content().clone();
        let undo = editor.session.history_status().undo;
        editor.begin_fill()?;
        editor.fill([0.2, 0.4, 0.6, 0.8])?;
        let first_preview = editor.snapshot().revision();
        for _ in 0..4 {
            editor.fill([0.2, 0.4, 0.6, 0.8])?;
            assert_eq!(editor.snapshot().revision(), first_preview);
            assert_eq!(editor.session.history_status().undo, undo);
        }
        assert_eq!(editor.session.snapshot().revision(), original.revision());
        let before_invalid = editor.fill;
        for invalid in [
            [f64::NAN, 0., 0., 1.],
            [1.1, 0., 0., 1.],
            [0., 0., 0., -0.1],
        ] {
            assert!(editor.fill(invalid).is_err());
            assert_eq!(editor.fill, before_invalid);
            assert_eq!(editor.snapshot().revision(), first_preview);
        }
        let final_srgb = [0.7, 0.3, 0.1, 0.5];
        editor.fill(final_srgb)?;
        assert_ne!(editor.snapshot().revision(), first_preview);
        editor.commit()?;
        assert_eq!(editor.session.history_status().undo, undo + 1);
        let changed_content = editor
            .snapshot()
            .node(node)
            .ok_or("No changed vector.")?
            .content()
            .clone();
        let Content::Vector(vector) = &changed_content else {
            return Err("Expected vector.".into());
        };
        assert_eq!(vector.fill(), Color::try_from(srgb_fill(final_srgb))?);
        let Content::Vector(before) = &original_content else {
            return Err("Expected original vector.".into());
        };
        assert_eq!(vector.path().elements(), before.path().elements());
        assert_eq!(
            vector.path().elements().as_ptr(),
            before.path().elements().as_ptr()
        );
        for (direction, expected) in [
            (History::Undo, &original_content),
            (History::Redo, &changed_content),
        ] {
            editor.history(direction)?;
            let restored = editor.snapshot();
            let Content::Vector(actual) =
                restored.node(node).ok_or("No restored vector.")?.content()
            else {
                return Err("Expected restored vector.".into());
            };
            let Content::Vector(expected) = expected else {
                return Err("Expected reference vector.".into());
            };
            assert_eq!(actual.fill(), expected.fill());
            assert_eq!(actual.path().elements(), expected.path().elements());
        }
        let revision = editor.snapshot().revision();
        let foreground = editor.fill;
        editor.begin_fill()?;
        editor.fill([0.9, 0.8, 0.7, 1.])?;
        editor.cancel()?;
        assert_eq!(editor.snapshot().revision(), revision);
        assert_eq!(editor.fill, foreground);
        assert_eq!(editor.session.history_status().undo, undo + 1);
        for (id, source) in original.assets() {
            let retained = editor.snapshot();
            let current = retained.asset(*id).ok_or("No original raster.")?;
            assert_eq!(current.pixels(), source.pixels());
            assert_eq!(current.pixels().as_ptr(), source.pixels().as_ptr());
        }
        Ok(())
    }

    #[test]
    fn raster_foreground_selection_never_recolors_pixels_or_marks_saved_artwork_dirty()
    -> AppResult<()> {
        let demo = Editor::demo()?;
        let mut editor = Editor::from_loaded(demo.session, PathBuf::from("saved.affint"));
        let original = editor.snapshot();
        let raster = original
            .layers()
            .find_map(|(node, layer)| {
                matches!(layer.content(), Content::Raster { .. }).then_some(*node)
            })
            .ok_or("No original raster.")?;
        editor.select(raster)?;
        let undo = editor.session.history_status().undo;
        editor.begin_fill()?;
        editor.fill([0.25, 0.5, 0.75, 0.8])?;
        assert!(!editor.dirty());
        assert_eq!(editor.snapshot().revision(), original.revision());
        editor.commit()?;
        let foreground = editor.fill;
        editor.begin_fill()?;
        editor.fill([0.75, 0.5, 0.25, 1.])?;
        editor.cancel()?;
        assert_eq!(editor.fill, foreground);
        assert!(!editor.dirty());
        assert_eq!(editor.session.history_status().undo, undo);
        for (id, source) in original.assets() {
            let snapshot = editor.snapshot();
            let retained = snapshot.asset(*id).ok_or("No original pixels.")?;
            assert_eq!(retained.pixels(), source.pixels());
            assert_eq!(retained.pixels().as_ptr(), source.pixels().as_ptr());
        }
        let before = editor.fill;
        assert!(editor.set_fill([f64::INFINITY, 0., 0., 1.]).is_err());
        assert_eq!(editor.fill, before);
        assert_eq!(editor.snapshot().revision(), original.revision());
        Ok(())
    }
}
