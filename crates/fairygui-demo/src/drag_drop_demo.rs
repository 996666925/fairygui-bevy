//! Exported Basics drag source/receiver, bound through ECS components and observers.
use crate::ui::basics::DemoDragDrop;
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
        if root.item_name != DemoDragDrop::ITEM_NAME {
            continue;
        }
        let Ok(binding) = DemoDragDrop::bind(&gui, page) else {
            continue;
        };
        let source = binding.b.entity();
        let receiver = binding.c.entity();
        let Some(icon) = gui.icon(source) else {
            continue;
        };
        let free = binding.a.entity();
        let bounded = binding.d.entity();
        let bounds = binding.bounds.entity();
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
