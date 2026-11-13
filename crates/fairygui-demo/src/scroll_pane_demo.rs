//! Nested horizontal action rows inside an ECS virtual vertical list.
use bevy::prelude::*;
use fairygui_bevy::core::object::EventKind;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;
#[derive(Component)]
pub(crate) struct RowBound;
#[derive(Component)]
pub(crate) struct List {
    label: Entity,
}
#[derive(Component)]
struct Action {
    row: Entity,
    label: Entity,
    verb: &'static str,
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    rows: Query<(Entity, &GuiListRow), Without<RowBound>>,
    lists: Query<&List>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Main"
            || !source
                .asset
                .path()
                .is_some_and(|path| path.path().ends_with("ScrollPane.fui"))
        {
            continue;
        }
        let Some(list) = gui.child_by_name(page, "list") else {
            continue;
        };
        let Some(label) = gui.child_by_name(page, "txt") else {
            continue;
        };
        let Some(width) = gui.scroll(list).map(|scroll| scroll.view_size().x) else {
            continue;
        };
        commands
            .entity(list)
            .insert((
                List { label },
                GuiVirtualList::new(1000, move |index, row| {
                    row.set_size(Vec2::new(width, row.size().y));
                    row.set_text(&format!("Item {index}"));
                    row.set_scroll_position(Vec2::ZERO);
                }),
            ))
            .observe(close_open_rows);
        commands.entity(page).insert(Bound);
    }
    for (row, membership) in &rows {
        let Ok(list) = lists.get(membership.list()) else {
            continue;
        };
        let Some(stick) = gui.child_by_name(row, "b0") else {
            continue;
        };
        let Some(delete) = gui.child_by_name(row, "b1") else {
            continue;
        };
        for (button, verb) in [(stick, "Stick"), (delete, "Delete")] {
            commands
                .entity(button)
                .insert(Action {
                    row,
                    label: list.label,
                    verb,
                })
                .observe(action);
        }
        commands.entity(row).insert(RowBound);
    }
}

fn descendant(gui: &GuiQuery, mut target: Entity, ancestor: Entity) -> bool {
    loop {
        if target == ancestor {
            return true;
        }
        let Some(parent) = gui.get(target).and_then(|object| object.parent()) else {
            return false;
        };
        target = parent;
    }
}

fn close_open_rows(
    event: On<GuiEvent>,
    gui: GuiQuery,
    mut commands: Commands,
    mut gui_edit: GuiMut,
) {
    if event.event.kind != EventKind::MouseDown {
        return;
    }
    let Some(target) = event.target else { return };
    let Some(list) = gui.list(event.entity) else {
        return;
    };
    for &row in list.items() {
        if gui
            .scroll(row)
            .is_some_and(|scroll| scroll.position().x != 0.)
        {
            if ["b0", "b1"].iter().any(|name| {
                gui.child_by_name(row, name)
                    .is_some_and(|button| descendant(&gui, target, button))
            }) {
                return;
            }
            gui_edit.edit(row, |object: &mut GuiObjectMut| {
                object.scroll_to(Vec2::ZERO, true)
            });
            commands.write_message(GuiInputCommand::CancelScrollDrag(row));
            commands.write_message(GuiInputCommand::CancelScrollDrag(event.entity));
            break;
        }
    }
}

fn action(event: On<GuiEvent>, actions: Query<&Action>, gui: GuiQuery, mut gui_edit: GuiMut) {
    if event.event.kind != EventKind::Click {
        return;
    }
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    gui_edit.edit(action.label, |object: &mut GuiObjectMut| {
        object.set_text(format!(
            "{} {}",
            action.verb,
            gui.text(action.row).unwrap_or_default()
        ))
    });
}
