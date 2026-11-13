//! Original five-icon loop carousel, driven by ECS list and scroll snapshots.
use bevy::prelude::*;
use fairygui_bevy::prelude::*;
use fairygui_core::package::FuiPackage;

#[derive(Component)]
pub(crate) struct Carousel {
    label: Entity,
}

pub fn initialize(
    commands: &mut Commands,
    gui: &GuiQuery,
    root: Entity,
    package: &FuiPackage,
) -> bool {
    let Some((list, label)) = gui
        .child_by_name(root, "list")
        .zip(gui.child_by_name(root, "n3"))
    else {
        return false;
    };
    let icons: Vec<_> = (1..=5)
        .map(|index| {
            let item = package
                .item(
                    package
                        .item_index_by_name(&format!("n{index}"))
                        .expect("LoopList icon"),
                )
                .unwrap();
            format!("ui://{}{}", package.id(), item.id)
        })
        .collect();
    commands.entity(list).insert((
        Carousel { label },
        GuiVirtualList::new(icons.len(), move |index, row| {
            row.set_pivot(Vec2::splat(0.5), false);
            row.set_icon(&icons[index]);
        })
        .looping(),
    ));
    true
}

/// PostLayout sees this frame's retained rows, indices and logical geometry.
pub fn update(
    gui: GuiQuery,
    lists: Query<(&Carousel, &GuiListState, &GuiScrollState)>,
    mut objects: GuiMut,
) {
    for (carousel, list, scroll) in &lists {
        let mid = scroll.position().x + scroll.view_size().x / 2.;
        for &row in list.items() {
            let Some(transform) = gui.transform(row) else {
                continue;
            };
            let width = transform.size().x;
            let distance = (mid - transform.position().x - width / 2.).abs();
            let scale = if width <= 0. || distance > width {
                1.
            } else {
                1. + (1. - distance / width) * 0.24
            };
            if transform.scale() != Vec2::splat(scale) {
                objects.edit(row, |object: &mut GuiObjectMut| {
                    object.set_scale(Vec2::splat(scale))
                });
            }
        }
        if list.item_count() > 0
            && let Some(first) = list.first_item_in_view()
        {
            let text = ((first + 1) % list.item_count()).to_string();
            if gui.text(carousel.label) != Some(text.as_str()) {
                objects.edit(carousel.label, |object: &mut GuiObjectMut| {
                    object.set_text(text)
                });
            }
        }
    }
}
