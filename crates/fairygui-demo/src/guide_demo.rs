//! Retained guide overlay, native coordinate queries and property tweens.
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fairygui_bevy::core::{Rect, object::EventKind};
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;
#[derive(Component)]
pub(crate) struct Guide {
    pub layer: Entity,
    bag: Entity,
    pending: bool,
}
#[derive(Component)]
struct Button {
    page: Entity,
    show: bool,
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
                .is_some_and(|path| path.path().ends_with("Guide.fui"))
        {
            continue;
        }
        let Some(bag) = gui.child_by_name(page, "bagBtn") else {
            continue;
        };
        let Some(start) = gui.child_by_name(page, "n2") else {
            continue;
        };
        let mut layer = FairyGuiRoot::new(source.asset.clone(), "GuideLayer");
        layer.active = false;
        let layer = commands.spawn((layer, ChildOf(page))).id();
        commands.entity(page).insert((
            Bound,
            Guide {
                layer,
                bag,
                pending: false,
            },
        ));
        commands
            .entity(start)
            .insert(Button { page, show: true })
            .observe(click);
        commands
            .entity(bag)
            .insert(Button { page, show: false })
            .observe(click);
    }
}

fn click(
    event: On<GuiEvent>,
    buttons: Query<&Button>,
    mut pages: Query<&mut Guide>,
    mut roots: Query<&mut FairyGuiRoot>,
) {
    if event.event.kind != EventKind::Click {
        return;
    }
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let Ok(mut guide) = pages.get_mut(button.page) else {
        return;
    };
    guide.pending = button.show;
    if !button.show
        && let Ok(mut layer) = roots.get_mut(guide.layer)
    {
        layer.active = false;
    }
}

pub fn update(
    mut pages: Query<(Entity, &mut Guide)>,
    mut roots: Query<&mut FairyGuiRoot>,
    windows: Query<&Window, With<PrimaryWindow>>,
    gui: GuiQuery,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    let Ok(window) = windows.single() else { return };
    let size = Vec2::new(window.width(), window.height());
    for (page, mut guide) in &mut pages {
        let mut resizing = false;
        for target in [page, guide.layer] {
            let Some(transform) = gui.transform(target) else {
                resizing = true;
                continue;
            };
            if transform.size() != size {
                commands.write(GuiObjectCommand::new(
                    target,
                    GuiObjectAction::SetSize(size),
                ));
                resizing = true;
            }
            if transform.position() != Vec2::ZERO {
                commands.write(GuiObjectCommand::new(
                    target,
                    GuiObjectAction::SetPosition(Vec2::ZERO),
                ));
                resizing = true;
            }
        }
        if resizing && roots.get(guide.layer).is_ok_and(|layer| layer.active) {
            guide.pending = true;
        }
        if !guide.pending || resizing {
            continue;
        }
        let Some(hole) = gui.child_by_name(guide.layer, "window") else {
            continue;
        };
        let Some(bag) = gui.transform(guide.bag) else {
            continue;
        };
        let Some(rect) = gui
            .local_to_global_rect(guide.bag, Rect::new(0., 0., bag.size().x, bag.size().y))
            .and_then(|rect| gui.global_to_local_rect(guide.layer, rect))
        else {
            continue;
        };
        if let Ok(mut layer) = roots.get_mut(guide.layer) {
            layer.active = true;
        }
        commands.write(GuiObjectCommand::new(
            hole,
            GuiObjectAction::SetSize(Vec2::new(rect.width, rect.height)),
        ));
        commands.write(GuiObjectCommand::new(
            hole,
            GuiObjectAction::Tween {
                target: GuiTweenTarget::Position(Vec2::new(rect.x, rect.y)),
                duration: 0.5,
                ease: 5,
            },
        ));
        guide.pending = false;
    }
}
