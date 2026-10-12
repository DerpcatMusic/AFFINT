use std::collections::BTreeSet;
use std::mem::size_of;
use std::sync::{Arc, atomic::AtomicU64};

use crate::session::{State, allocate};
use crate::validate::check_limit;
use crate::{
    AssetId, Change, Content, Created, Error, Layer, LayerSpec, Limits, NodeId, Page, PageId,
    Parent,
};

pub(crate) fn apply(
    base: &State,
    changes: Vec<Change>,
    watermark: &AtomicU64,
    limits: &Limits,
) -> Result<(State, Created), Error> {
    let mut state = base.clone();
    let mut created = Created::default();
    // Session already owns these typed inputs. Transfer their payloads rather
    // than making a second bulk allocation before final complete validation.
    for change in changes {
        match change {
            Change::AddPage { name, size } => {
                check_limit(
                    "pages",
                    state
                        .pages
                        .size()
                        .checked_add(1)
                        .ok_or(Error::Overflow("pages"))?,
                    limits.max_pages,
                )?;
                check_limit("name bytes", name.len(), limits.max_name_bytes)?;
                let id = PageId(allocate(watermark, "document identities")?);
                state.pages.insert_mut(
                    id,
                    Page {
                        name: Arc::from(name),
                        size,
                        roots: Arc::new(Vec::new()),
                    },
                );
                Arc::make_mut(&mut state.page_order).push(id);
                created.pages.push(id);
            }
            Change::SetPage { page, name, size } => {
                check_limit("name bytes", name.len(), limits.max_name_bytes)?;
                let mut value = state
                    .pages
                    .get(&page)
                    .cloned()
                    .ok_or(Error::MissingPage(page))?;
                value.name = Arc::from(name);
                value.size = size;
                state.pages.insert_mut(page, value);
            }
            Change::AddAsset { asset } => {
                let id = add_asset(&mut state, asset, watermark, limits)?;
                created.assets.push(id);
            }
            Change::InsertLayer {
                parent,
                index,
                layer,
            } => {
                let id = insert(&mut state, parent, index, layer, watermark, limits)?;
                created.nodes.push(id);
            }
            Change::InsertRaster {
                parent,
                index,
                name,
                asset,
                transform,
            } => {
                check_limit("name bytes", name.len(), limits.max_name_bytes)?;
                let asset = add_asset(&mut state, asset, watermark, limits)?;
                let mut layer = LayerSpec::new(name, Content::Raster { asset });
                layer.transform = transform;
                let node = insert(&mut state, parent, index, layer, watermark, limits)?;
                created.assets.push(asset);
                created.nodes.push(node);
            }
            Change::SetLayer { node, layer } => {
                check_spec(&layer, limits)?;
                let old = state.layers.get(&node).ok_or(Error::MissingLayer(node))?;
                let mut replacement = layer_from_spec(layer, old.parent, limits)?;
                replacement.children = Arc::clone(&old.children);
                state.layers.insert_mut(node, replacement);
            }
            Change::SetTransform { node, transform } => {
                modify(&mut state, node, |layer| layer.transform = transform)?
            }
            Change::SetMask { node, mask } => modify(&mut state, node, |layer| layer.mask = mask)?,
            Change::SetOperations { node, operations } => {
                check_limit("operations", operations.len(), limits.max_operations)?;
                let operations = take_operations(operations, limits)?;
                modify(&mut state, node, |layer| layer.operations = operations)?;
            }
            Change::MoveLayer {
                node,
                parent,
                index,
            } => {
                let old = state
                    .layers
                    .get(&node)
                    .ok_or(Error::MissingLayer(node))?
                    .parent;
                unlink(&mut state, old, node)?;
                link(&mut state, parent, index, node)?;
                modify(&mut state, node, |layer| layer.parent = parent)?;
            }
            Change::RemoveLayer { node } => {
                let parent = state
                    .layers
                    .get(&node)
                    .ok_or(Error::MissingLayer(node))?
                    .parent;
                let mut removed = BTreeSet::new();
                let mut stack = vec![node];
                while let Some(id) = stack.pop() {
                    if !removed.insert(id) {
                        return Err(Error::Cycle);
                    }
                    let layer = state.layers.get(&id).ok_or(Error::MissingLayer(id))?;
                    stack.extend_from_slice(layer.children());
                }
                unlink(&mut state, parent, node)?;
                for id in removed {
                    state.layers.remove_mut(&id);
                }
            }
        }
    }
    Ok((state, created))
}

fn add_asset(
    state: &mut State,
    asset: Arc<crate::RasterAsset>,
    watermark: &AtomicU64,
    limits: &Limits,
) -> Result<AssetId, Error> {
    check_limit(
        "assets",
        state
            .assets
            .size()
            .checked_add(1)
            .ok_or(Error::Overflow("assets"))?,
        limits.max_assets,
    )?;
    check_limit(
        "source pixel bytes",
        asset.pixel_owner().capacity(),
        limits.max_pixel_bytes,
    )?;
    let id = AssetId(allocate(watermark, "document identities")?);
    state.assets.insert_mut(id, asset);
    Ok(id)
}

fn insert(
    state: &mut State,
    parent: Parent,
    index: usize,
    spec: LayerSpec,
    watermark: &AtomicU64,
    limits: &Limits,
) -> Result<NodeId, Error> {
    check_limit(
        "layers",
        state
            .layers
            .size()
            .checked_add(1)
            .ok_or(Error::Overflow("layers"))?,
        limits.max_layers,
    )?;
    check_spec(&spec, limits)?;
    let id = NodeId(allocate(watermark, "document identities")?);
    link(state, parent, index, id)?;
    state
        .layers
        .insert_mut(id, layer_from_spec(spec, parent, limits)?);
    Ok(id)
}

fn check_spec(spec: &LayerSpec, limits: &Limits) -> Result<(), Error> {
    check_limit("name bytes", spec.name.len(), limits.max_name_bytes)?;
    check_limit("operations", spec.operations.len(), limits.max_operations)?;
    if let Content::Vector(vector) = &spec.content {
        check_limit(
            "path elements",
            vector.path().elements().len(),
            limits.max_path_elements,
        )?;
    }
    Ok(())
}

fn layer_from_spec(spec: LayerSpec, parent: Parent, limits: &Limits) -> Result<Layer, Error> {
    let operations = take_operations(spec.operations, limits)?;
    Ok(Layer {
        name: Arc::from(spec.name),
        parent,
        children: Arc::new(Vec::new()),
        content: spec.content,
        transform: spec.transform,
        opacity: spec.opacity,
        visible: spec.visible,
        mask: spec.mask,
        operations,
    })
}

fn take_operations(
    mut operations: Vec<crate::LiveOperation>,
    limits: &Limits,
) -> Result<Arc<Vec<crate::LiveOperation>>, Error> {
    let used_bytes = operations
        .len()
        .checked_mul(size_of::<crate::LiveOperation>())
        .ok_or(Error::Overflow("operation bytes"))?;
    // The old borrowed staging path cloned only live elements. Preserve admission
    // for small inputs with large spare capacity by shrinking their existing owner.
    // A possible realloc may only request a payload within the metadata policy.
    // Larger inputs move unchanged, allowing a later replacement in the same batch
    // without making an oversized second Vec or adding an intermediate-state refusal.
    if operations.capacity() > operations.len() && used_bytes <= limits.max_metadata_bytes {
        operations.shrink_to_fit();
    }
    Ok(Arc::new(operations))
}

fn modify(state: &mut State, node: NodeId, change: impl FnOnce(&mut Layer)) -> Result<(), Error> {
    let mut layer = state
        .layers
        .get(&node)
        .cloned()
        .ok_or(Error::MissingLayer(node))?;
    change(&mut layer);
    state.layers.insert_mut(node, layer);
    Ok(())
}

fn link(state: &mut State, parent: Parent, index: usize, node: NodeId) -> Result<(), Error> {
    match parent {
        Parent::Page(id) => {
            let mut page = state
                .pages
                .get(&id)
                .cloned()
                .ok_or(Error::MissingPage(id))?;
            insert_id(Arc::make_mut(&mut page.roots), index, node)?;
            state.pages.insert_mut(id, page);
        }
        Parent::Layer(id) => {
            let mut layer = state
                .layers
                .get(&id)
                .cloned()
                .ok_or(Error::MissingLayer(id))?;
            if !matches!(layer.content, Content::Group) {
                return Err(Error::NotAGroup(id));
            }
            insert_id(Arc::make_mut(&mut layer.children), index, node)?;
            state.layers.insert_mut(id, layer);
        }
    }
    Ok(())
}

fn insert_id(list: &mut Vec<NodeId>, index: usize, node: NodeId) -> Result<(), Error> {
    if index > list.len() {
        return Err(Error::InvalidIndex {
            index,
            len: list.len(),
        });
    }
    list.insert(index, node);
    Ok(())
}

fn unlink(state: &mut State, parent: Parent, node: NodeId) -> Result<(), Error> {
    match parent {
        Parent::Page(id) => {
            let mut page = state
                .pages
                .get(&id)
                .cloned()
                .ok_or(Error::MissingPage(id))?;
            remove_id(Arc::make_mut(&mut page.roots), node)?;
            state.pages.insert_mut(id, page);
        }
        Parent::Layer(id) => {
            let mut layer = state
                .layers
                .get(&id)
                .cloned()
                .ok_or(Error::MissingLayer(id))?;
            remove_id(Arc::make_mut(&mut layer.children), node)?;
            state.layers.insert_mut(id, layer);
        }
    }
    Ok(())
}

fn remove_id(list: &mut Vec<NodeId>, node: NodeId) -> Result<(), Error> {
    let index = list
        .iter()
        .position(|id| *id == node)
        .ok_or(Error::ParentMismatch(node))?;
    list.remove(index);
    Ok(())
}
