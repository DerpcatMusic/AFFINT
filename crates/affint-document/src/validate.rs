use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::mem::size_of;

use kurbo::Affine;

use crate::session::State;
use crate::{Content, Error, Layer, Limits, LiveOperation, NodeId, Parent, Transform};

pub(crate) fn check_limit(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), Error> {
    if actual > limit {
        return Err(Error::LimitExceeded {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

pub(crate) fn validate(state: &State, limits: &Limits, watermark: u64) -> Result<usize, Error> {
    if watermark == 0 {
        return Err(Error::InvalidIdentity(0));
    }
    check_limit("pages", state.pages.size(), limits.max_pages)?;
    check_limit("layers", state.layers.size(), limits.max_layers)?;
    check_limit("assets", state.assets.size(), limits.max_assets)?;
    let mut identities = BTreeSet::new();
    let mut owners = BTreeMap::new();
    let mut metadata = size_of::<State>();
    add_bytes(
        &mut metadata,
        state.page_order.capacity(),
        size_of::<crate::PageId>(),
    )?;
    let mut page_order = BTreeSet::new();
    for id in state.page_order.iter() {
        if !state.pages.contains_key(id) {
            return Err(Error::MissingPage(*id));
        }
        if !page_order.insert(*id) {
            return Err(Error::InvalidIdentity(id.get()));
        }
    }
    if page_order.len() != state.pages.size() {
        return Err(Error::InvalidValue("page order"));
    }
    for (id, page) in state.pages.iter() {
        identity(&mut identities, id.get(), watermark)?;
        crate::PageSize::new(page.size.width(), page.size.height())?;
        check_limit("name bytes", page.name.len(), limits.max_name_bytes)?;
        // Map node, shared entry, Arc headers, and persistent-tree links. Dynamic
        // payload is charged separately; the allowance is intentionally conservative.
        add_bytes(&mut metadata, 1, size_of::<crate::Page>() + 256)?;
        add_bytes(&mut metadata, page.name.len(), 1)?;
        add_bytes(&mut metadata, page.roots.capacity(), size_of::<NodeId>())?;
        for node in page.roots.iter() {
            assign_owner(&mut owners, *node, Parent::Page(*id), state)?;
        }
    }
    for (id, asset) in state.assets.iter() {
        identity(&mut identities, id.get(), watermark)?;
        let expected = asset.size().byte_len()?;
        if asset.pixels().len() != expected {
            return Err(Error::PixelLength {
                expected,
                actual: asset.pixels().len(),
            });
        }
        check_limit(
            "source pixel bytes",
            asset.pixel_owner().capacity(),
            limits.max_pixel_bytes,
        )?;
        add_bytes(&mut metadata, 1, size_of::<crate::RasterAsset>() + 256)?;
    }
    let mut operations = 0usize;
    let mut path_elements = 0usize;
    for (id, layer) in state.layers.iter() {
        identity(&mut identities, id.get(), watermark)?;
        check_limit("name bytes", layer.name.len(), limits.max_name_bytes)?;
        if !layer.opacity.is_finite() || !(0.0..=1.0).contains(&layer.opacity) {
            return Err(Error::InvalidValue("layer opacity"));
        }
        Transform::new(layer.transform.affine().as_coeffs())?;
        match &layer.content {
            Content::Group => {}
            Content::Raster { asset } => {
                if !state.assets.contains_key(asset) {
                    return Err(Error::MissingAsset(*asset));
                }
                if !layer.children.is_empty() {
                    return Err(Error::NotAGroup(*id));
                }
            }
            Content::Vector(vector) => {
                if !layer.children.is_empty() {
                    return Err(Error::NotAGroup(*id));
                }
                if !vector.path().is_finite() {
                    return Err(Error::InvalidValue("vector coordinates"));
                }
                let count = vector.path().elements().len();
                path_elements = path_elements
                    .checked_add(count)
                    .ok_or(Error::Overflow("path elements"))?;
                // BezPath also owns its inline storage. Heap elements receive a
                // growth allowance; repeated shared instances are overcharged.
                add_bytes(&mut metadata, 1, size_of::<kurbo::BezPath>() + 32)?;
                add_bytes(&mut metadata, count, 2 * size_of::<kurbo::PathEl>())?;
            }
        }
        for child in layer.children.iter() {
            assign_owner(&mut owners, *child, Parent::Layer(*id), state)?;
        }
        if let Some(mask) = layer.mask {
            if !state.layers.contains_key(&mask.source) {
                return Err(Error::LayerInUse {
                    target: mask.source,
                    user: *id,
                });
            }
        }
        operations = operations
            .checked_add(layer.operations.len())
            .ok_or(Error::Overflow("operations"))?;
        for operation in layer.operations.iter() {
            validate_operation(operation)?;
        }
        add_bytes(&mut metadata, 1, size_of::<Layer>() + 256)?;
        add_bytes(&mut metadata, layer.name.len(), 1)?;
        add_bytes(
            &mut metadata,
            layer.children.capacity(),
            size_of::<NodeId>(),
        )?;
        add_bytes(
            &mut metadata,
            layer.operations.capacity(),
            size_of::<LiveOperation>(),
        )?;
    }
    check_limit("operations", operations, limits.max_operations)?;
    check_limit("path elements", path_elements, limits.max_path_elements)?;
    check_limit("state metadata bytes", metadata, limits.max_metadata_bytes)?;
    for (id, layer) in state.layers.iter() {
        if owners.get(id) != Some(&layer.parent) {
            return Err(Error::ParentMismatch(*id));
        }
    }
    validate_dependencies(state, limits)?;
    validate_world_placements(state, limits)?;
    Ok(metadata)
}

fn identity(ids: &mut BTreeSet<u64>, id: u64, watermark: u64) -> Result<(), Error> {
    if id == 0 || id >= watermark || !ids.insert(id) {
        return Err(Error::InvalidIdentity(id));
    }
    Ok(())
}

fn add_bytes(bytes: &mut usize, count: usize, size: usize) -> Result<(), Error> {
    *bytes = count
        .checked_mul(size)
        .and_then(|added| bytes.checked_add(added))
        .ok_or(Error::Overflow("metadata bytes"))?;
    Ok(())
}

fn assign_owner(
    owners: &mut BTreeMap<NodeId, Parent>,
    node: NodeId,
    parent: Parent,
    state: &State,
) -> Result<(), Error> {
    if !state.layers.contains_key(&node) {
        return Err(Error::MissingLayer(node));
    }
    if owners.insert(node, parent).is_some() {
        return Err(Error::ParentMismatch(node));
    }
    Ok(())
}

fn validate_operation(operation: &LiveOperation) -> Result<(), Error> {
    match operation {
        LiveOperation::Crop(rect) => {
            if ![
                rect.x0,
                rect.y0,
                rect.x1,
                rect.y1,
                rect.width(),
                rect.height(),
            ]
            .iter()
            .all(|value| value.is_finite())
                || rect.width() <= 0.0
                || rect.height() <= 0.0
            {
                return Err(Error::InvalidValue("live crop rectangle"));
            }
        }
        LiveOperation::Exposure(stops) => {
            if !stops.is_finite() || !(-32.0..=32.0).contains(stops) {
                return Err(Error::InvalidValue("live exposure stops"));
            }
        }
    }
    Ok(())
}

fn dependencies(layer: &Layer) -> impl Iterator<Item = NodeId> + '_ {
    layer
        .children
        .iter()
        .copied()
        .chain(layer.mask.map(|mask| mask.source))
}

fn validate_dependencies(state: &State, limits: &Limits) -> Result<(), Error> {
    let mut incoming: BTreeMap<NodeId, usize> = state.layers.keys().map(|id| (*id, 0)).collect();
    let mut depths: BTreeMap<NodeId, usize> = state.layers.keys().map(|id| (*id, 1)).collect();
    for (_, layer) in state.layers.iter() {
        for dependency in dependencies(layer) {
            let degree = incoming
                .get_mut(&dependency)
                .ok_or(Error::MissingLayer(dependency))?;
            *degree = degree
                .checked_add(1)
                .ok_or(Error::Overflow("dependency edges"))?;
        }
    }
    let mut queue: VecDeque<NodeId> = incoming
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect();
    let mut visited = 0usize;
    while let Some(id) = queue.pop_front() {
        visited = visited
            .checked_add(1)
            .ok_or(Error::Overflow("visited layers"))?;
        let depth = *depths.get(&id).ok_or(Error::MissingLayer(id))?;
        check_limit("dependency depth", depth, limits.max_depth)?;
        let layer = state.layers.get(&id).ok_or(Error::MissingLayer(id))?;
        for dependency in dependencies(layer) {
            let next_depth = depth
                .checked_add(1)
                .ok_or(Error::Overflow("dependency depth"))?;
            let stored_depth = depths
                .get_mut(&dependency)
                .ok_or(Error::MissingLayer(dependency))?;
            *stored_depth = (*stored_depth).max(next_depth);
            let degree = incoming
                .get_mut(&dependency)
                .ok_or(Error::MissingLayer(dependency))?;
            *degree = degree
                .checked_sub(1)
                .ok_or(Error::InvalidValue("dependency degree"))?;
            if *degree == 0 {
                queue.push_back(dependency);
            }
        }
    }
    if visited != state.layers.size() {
        return Err(Error::Cycle);
    }
    Ok(())
}

fn validate_world_placements(state: &State, limits: &Limits) -> Result<(), Error> {
    let mut stack = Vec::new();
    for (_, page) in state.pages.iter() {
        stack.extend(page.roots.iter().map(|id| (*id, Affine::IDENTITY, 1usize)));
    }
    let mut visited = 0usize;
    while let Some((id, parent, depth)) = stack.pop() {
        visited = visited
            .checked_add(1)
            .ok_or(Error::Overflow("tree layers"))?;
        check_limit("tree depth", depth, limits.max_depth)?;
        let layer = state.layers.get(&id).ok_or(Error::MissingLayer(id))?;
        let world = parent * layer.transform.affine();
        Transform::new(world.as_coeffs())?;
        let next_depth = depth.checked_add(1).ok_or(Error::Overflow("tree depth"))?;
        stack.extend(
            layer
                .children
                .iter()
                .map(|child| (*child, world, next_depth)),
        );
    }
    if visited != state.layers.size() {
        return Err(Error::Cycle);
    }
    Ok(())
}
