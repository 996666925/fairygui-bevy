//! Bevy must retain the native model's empty tail as scrollable layout space.
use super::*;

#[derive(Component, Debug)]
pub(crate) struct ScrollExtent(pub GObjectId);

pub(super) fn node(
    object: &fairygui_core::GObject,
    pane: &fairygui_core::scroll::ScrollPane,
) -> Node {
    let max = pane.max_scroll_pos();
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.),
        top: Val::Px(0.),
        width: Val::Px(object.width() + max.x),
        height: Val::Px(object.height() + max.y),
        ..default()
    }
}

pub(crate) fn sync_scroll_extents(
    runtime: NonSend<GuiRuntime>,
    mut nodes: Query<(&ScrollExtent, &mut Node)>,
) {
    for (extent, mut current) in &mut nodes {
        let Some(object) = runtime.store().object(extent.0) else {
            continue;
        };
        let Some(pane) = runtime.store().scroll_pane(extent.0) else {
            continue;
        };
        let next = node(object, pane);
        if current.width != next.width || current.height != next.height {
            current.width = next.width;
            current.height = next.height;
        }
    }
}
