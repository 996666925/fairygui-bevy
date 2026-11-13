//! Mail rows and staggered exported transitions, configured through ECS.
use crate::mail_item::MailItem;
use bevy::prelude::*;

#[cfg(test)]
#[path = "list_effect_demo_tests.rs"]
mod tests;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;
#[derive(Component)]
pub(crate) struct Pending;

pub(crate) fn content() -> GuiListContent {
    GuiListContent::new(10, |index, row| {
        row.set_controller_page("c1", i32::from(index % 3 == 0));
        row.set_controller_page("IsRead", i32::from(index % 2 == 0));
        row.set_text("Mail title here");
        if let Some(mut time) = row.child_by_path("timeText") {
            time.set_text("5 Nov 2015 16:24:33");
        }
    })
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Main"
            || !source
                .asset
                .path()
                .is_some_and(|path| path.path().ends_with("ListEffect.fui"))
        {
            continue;
        }
        let Some(list) = gui.child_by_name(page, "mailList") else {
            continue;
        };
        commands.entity(list).insert((Pending, content()));
        commands.entity(page).insert(Bound);
    }
}

/// Use native geometry after list preparation and before final projection, so
/// hiding delayed rows never flashes a fully visible unanimated first frame.
pub fn play(
    mut commands: Commands,
    mut gui_edit: GuiMut,
    lists: Query<(Entity, &GuiListState), With<Pending>>,
    mail_items: Query<&MailItem>,
    gui: GuiQuery,
) {
    for (list, content) in &lists {
        if content.item_count() != 10 || content.items().len() != 10 {
            continue;
        }
        for (index, &row) in content.items().iter().enumerate() {
            if gui.is_child_in_view(list, row) != Some(true) {
                break;
            }
            let Ok(mail) = mail_items.get(row) else {
                continue;
            };
            debug_assert!(gui.text(mail.time).is_some());
            mail.play_effect(&mut gui_edit, index as f32 * 0.2);
        }
        commands.entity(list).remove::<Pending>();
    }
}
