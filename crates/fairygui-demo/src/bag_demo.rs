//! Native bag window with virtual inventory and per-list entity observers.
use bevy::prelude::*;
use fairygui_bevy::prelude::*;
use std::hash::BuildHasher;

#[derive(Component)]
pub(crate) struct Bound;
#[derive(Component)]
pub(crate) struct BagWindow;
#[derive(Component)]
struct OpenBag(Entity);
#[derive(Component)]
struct Preview {
    icon: Entity,
    label: Entity,
}

fn inventory() -> GuiVirtualList {
    let random = std::collections::hash_map::RandomState::new();
    let items: Vec<_> = (0..45)
        .map(|index| {
            let value = random.hash_one(index);
            (
                format!("icons/i{}.png", value % 10),
                ((value / 10) % 100).to_string(),
            )
        })
        .collect();
    GuiVirtualList::new(items.len(), move |index, row| {
        row.set_icon(&items[index].0);
        row.set_text(&items[index].1);
    })
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    windows: Query<Entity, (With<BagWindow>, Without<Bound>)>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Main"
            || !source
                .asset
                .path()
                .is_some_and(|path| path.path().ends_with("Bag.fui"))
        {
            continue;
        }
        let Some(button) = gui.child_by_name(page, "bagBtn") else {
            continue;
        };
        let mut window = FairyGuiWindow::new(source.asset.clone(), "BagWin");
        window.visible = false;
        let owner = commands.spawn((window, BagWindow, ChildOf(page))).id();
        commands.entity(button).insert(OpenBag(owner)).observe(open);
        commands.entity(page).insert(Bound);
    }
    for owner in &windows {
        let Some(list) = gui.child_by_name(owner, "list") else {
            continue;
        };
        let Some(icon) = gui.child_by_name(owner, "n11") else {
            continue;
        };
        let Some(label) = gui.child_by_name(owner, "n13") else {
            continue;
        };
        commands
            .entity(list)
            .insert((inventory(), Preview { icon, label }))
            .observe(select);
        commands.entity(owner).insert(Bound);
    }
}

fn open(event: On<GuiEvent>, buttons: Query<&OpenBag>, gui: GuiQuery, mut commands: Commands) {
    if event.event.kind != fairygui_bevy::core::object::EventKind::Click {
        return;
    }
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    if let Some(list) = gui.child_by_name(button.0, "list") {
        commands.entity(list).insert(inventory());
    }
    commands.write_message(GuiWindowCommand::new(button.0, GuiWindowAction::Show));
    commands.write_message(GuiWindowCommand::new(
        button.0,
        GuiWindowAction::BringToFront,
    ));
}

fn select(
    event: On<GuiListItemClick>,
    previews: Query<&Preview>,
    mut properties: ParamSet<(Query<&GuiIcon>, Query<&mut GuiIcon>, Query<&mut GuiText>)>,
) {
    let Ok(preview) = previews.get(event.entity) else {
        return;
    };
    let Ok(icon) = properties.p0().get(event.item).map(|icon| icon.0.clone()) else {
        return;
    };
    if let Ok(mut target) = properties.p1().get_mut(preview.icon) {
        target.set(icon.clone());
    }
    if let Ok(mut target) = properties.p2().get_mut(preview.label) {
        target.set(icon);
    }
}
