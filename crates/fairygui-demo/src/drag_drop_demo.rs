//! Exported Basics drag source/receiver, bound through ECS components and observers.
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    gui: GuiQuery,
    mut gui_edit: GuiMut,
) {
    for (page, root) in &pages {
        if root.item_name != "Demo_Drag&Drop" {
            continue;
        }
        let Some(source) = gui.child_by_name(page, "b") else {
            continue;
        };
        let Some(receiver) = gui.child_by_name(page, "c") else {
            continue;
        };
        let Some(icon) = gui.icon(source) else {
            continue;
        };
        let Some(free) = gui.child_by_name(page, "a") else {
            continue;
        };
        let Some(bounded) = gui.child_by_name(page, "d") else {
            continue;
        };
        let Some(bounds) = gui.child_by_name(page, "bounds") else {
            continue;
        };
        commands.entity(free).insert(GuiDraggable::default());
        commands.entity(bounded).insert(GuiDraggable {
            bounds: Some(GuiDragBounds::Control(bounds)),
        });
        commands.entity(source).insert(GuiDragSource {
            icon: Some(icon.to_owned()),
            data: Some(icon.to_owned()),
        });
        commands
            .entity(receiver)
            .insert(GuiDropTarget)
            .observe(receive);
        gui_edit.edit(receiver, |object: &mut GuiObjectMut| {
            object.set_icon(String::new())
        });
        commands.entity(page).insert(Bound);
    }
}

fn receive(event: On<GuiDrop>, mut icons: Query<&mut GuiIcon>) {
    if let Some(icon) = &event.data
        && let Ok(mut target) = icons.get_mut(event.entity)
    {
        target.set(icon.clone());
    }
}
