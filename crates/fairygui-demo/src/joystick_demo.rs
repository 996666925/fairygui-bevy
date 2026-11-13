//! Joystick ownership lives on the page; only its touch area observes pointers.
use bevy::{prelude::*, window::PrimaryWindow};
use fairygui_bevy::{core::object::EventKind, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pointer {
    Mouse,
    Touch(u64),
}
impl Pointer {
    fn from_touch(id: Option<u64>) -> Self {
        id.map_or(Self::Mouse, Self::Touch)
    }
}

#[derive(Component)]
pub(crate) struct Joystick {
    pub button: Entity,
    pub center: Entity,
    pub thumb: Entity,
    pub output: Entity,
    pub area: Entity,
    pub home: Vec2,
    viewport: Vec2,
    pointer: Option<Pointer>,
    start: Vec2,
    last: Vec2,
    button_center: Vec2,
    rotation: f32,
    pub returning: bool,
}

#[derive(Component)]
struct Owner(Entity);

#[derive(EntityEvent)]
struct Moving {
    entity: Entity,
    degrees: f32,
}
#[derive(EntityEvent)]
struct Released {
    entity: Entity,
}

fn source(root: &FairyGuiRoot) -> bool {
    root.item_name == "Main"
        && root
            .asset
            .path()
            .is_some_and(|p| p.path().ends_with("Joystick.fui"))
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Joystick>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    gui: GuiQuery,
) {
    let Ok(window) = windows.single() else { return };
    let viewport = Vec2::new(window.width(), window.height());
    for (page, root) in &pages {
        if !source(root)
            || !gui
                .transform(page)
                .is_some_and(|t| t.size() == viewport && t.position() == Vec2::ZERO)
        {
            continue;
        }
        let (Some(button), Some(center), Some(area), Some(output)) = (
            gui.child_by_name(page, "joystick"),
            gui.child_by_name(page, "joystick_center"),
            gui.child_by_name(page, "joystick_touch"),
            gui.child_by_name(page, "n9"),
        ) else {
            continue;
        };
        let Some(thumb) = gui.child_by_name(button, "thumb") else {
            continue;
        };
        let Some(transform) = gui.transform(center) else {
            continue;
        };
        let home = transform.position() + transform.size() / 2.;
        commands
            .entity(page)
            .insert(Joystick {
                button,
                center,
                thumb,
                output,
                area,
                home,
                viewport,
                pointer: None,
                start: home,
                last: home,
                button_center: home,
                rotation: 0.,
                returning: false,
            })
            .observe(moving)
            .observe(released);
        commands.entity(area).insert(Owner(page)).observe(pointer);
        commands
            .entity(button)
            .insert(Owner(page))
            .observe(returned);
        commands.write_message(GuiObjectCommand::new(
            button,
            GuiObjectAction::SetButtonChangeStateOnClick(false),
        ));
        commands.write_message(GuiObjectCommand::new(
            button,
            GuiObjectAction::SetButtonSelected(false),
        ));
    }
}

/// Like the reference, the joystick page fills the viewport. Resizing cancels
/// an active gesture and restores the bottom-relative resting position.
pub fn fit(
    mut pages: Query<(Entity, &FairyGuiRoot, Option<&mut Joystick>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    gui: GuiQuery,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    let Ok(window) = windows.single() else { return };
    let size = Vec2::new(window.width(), window.height());
    for (page, root, joystick) in &mut pages {
        if !source(root) {
            continue;
        }
        let Some(transform) = gui.transform(page) else {
            continue;
        };
        if transform.position() != Vec2::ZERO {
            commands.write(GuiObjectCommand::new(
                page,
                GuiObjectAction::SetPosition(Vec2::ZERO),
            ));
        }
        if transform.size() != size {
            commands.write(GuiObjectCommand::new(page, GuiObjectAction::SetSize(size)));
        }
        if let Some(mut joystick) = joystick
            && joystick.viewport != size
        {
            joystick.home.y += size.y - joystick.viewport.y;
            joystick.viewport = size;
            joystick.pointer = None;
            commands.write(GuiObjectCommand::new(
                joystick.button,
                GuiObjectAction::StopTween(GuiTweenProperty::Xy),
            ));
            restore(&mut joystick, &gui, &mut commands);
            commands.write(GuiObjectCommand::new(
                joystick.output,
                GuiObjectAction::SetText(String::new()),
            ));
        }
    }
}

fn position(
    entity: Entity,
    center: Vec2,
    gui: &GuiQuery,
    commands: &mut MessageWriter<GuiObjectCommand>,
) {
    if let Some(transform) = gui.transform(entity) {
        commands.write(GuiObjectCommand::new(
            entity,
            GuiObjectAction::SetPosition(center - transform.size() / 2.),
        ));
    }
}

fn restore(
    joystick: &mut Joystick,
    gui: &GuiQuery,
    commands: &mut MessageWriter<GuiObjectCommand>,
) {
    joystick.returning = false;
    joystick.rotation = 0.;
    joystick.button_center = joystick.home;
    position(joystick.button, joystick.home, gui, commands);
    position(joystick.center, joystick.home, gui, commands);
    for (entity, action) in [
        (joystick.button, GuiObjectAction::SetButtonSelected(false)),
        (joystick.thumb, GuiObjectAction::SetRotation(0.)),
        (joystick.center, GuiObjectAction::SetVisible(true)),
    ] {
        commands.write(GuiObjectCommand::new(entity, action));
    }
}

fn pointer(
    event: On<GuiEvent>,
    owners: Query<&Owner>,
    mut pages: Query<&mut Joystick>,
    gui: GuiQuery,
    mut messages: MessageWriter<GuiObjectCommand>,
    mut commands: Commands,
) {
    let Ok(owner) = owners.get(event.entity) else {
        return;
    };
    let Ok(mut joystick) = pages.get_mut(owner.0) else {
        return;
    };
    let id = Pointer::from_touch(event.event.touch_id);
    match event.event.kind {
        EventKind::MouseDown if joystick.pointer.is_none() => {
            let Some(point) = event
                .event
                .pointer
                .and_then(|p| gui.global_to_local(owner.0, Vec2::new(p.x, p.y)))
            else {
                return;
            };
            let Some(area) = gui.transform(joystick.area) else {
                return;
            };
            let point = Vec2::new(
                point.x.clamp(0., area.size().x),
                point.y.clamp(
                    area.position().y.min(joystick.viewport.y),
                    joystick.viewport.y,
                ),
            );
            joystick.pointer = Some(id);
            joystick.returning = false;
            joystick.start = point;
            joystick.last = point;
            joystick.button_center = point;
            joystick.rotation = (point - joystick.home).to_angle().to_degrees() + 90.;
            messages.write(GuiObjectCommand::new(
                joystick.button,
                GuiObjectAction::StopTween(GuiTweenProperty::Xy),
            ));
            messages.write(GuiObjectCommand::new(
                joystick.button,
                GuiObjectAction::SetButtonSelected(true),
            ));
            messages.write(GuiObjectCommand::new(
                joystick.center,
                GuiObjectAction::SetVisible(true),
            ));
            position(joystick.center, point, &gui, &mut messages);
            position(joystick.button, point, &gui, &mut messages);
            messages.write(GuiObjectCommand::new(
                joystick.thumb,
                GuiObjectAction::SetRotation(joystick.rotation),
            ));
        }
        EventKind::MouseDrag if joystick.pointer == Some(id) => {
            let Some(point) = event
                .event
                .pointer
                .and_then(|p| gui.global_to_local(owner.0, Vec2::new(p.x, p.y)))
            else {
                return;
            };
            let offset = joystick.button_center + point - joystick.last - joystick.start;
            joystick.last = point;
            let degrees = offset.to_angle().to_degrees();
            joystick.rotation = degrees + 90.;
            let mut center = joystick.start + offset.clamp_length_max(150.);
            center.x = center.x.max(0.);
            center.y = center.y.min(joystick.viewport.y);
            joystick.button_center = center;
            position(joystick.button, center, &gui, &mut messages);
            messages.write(GuiObjectCommand::new(
                joystick.thumb,
                GuiObjectAction::SetRotation(joystick.rotation),
            ));
            commands.trigger(Moving {
                entity: owner.0,
                degrees,
            });
        }
        EventKind::MouseUp if joystick.pointer == Some(id) => {
            joystick.pointer = None;
            joystick.returning = true;
            joystick.rotation += 180.;
            messages.write(GuiObjectCommand::new(
                joystick.thumb,
                GuiObjectAction::SetRotation(joystick.rotation),
            ));
            messages.write(GuiObjectCommand::new(
                joystick.center,
                GuiObjectAction::SetVisible(false),
            ));
            if let Some(button) = gui.transform(joystick.button) {
                messages.write(GuiObjectCommand::new(
                    joystick.button,
                    GuiObjectAction::Tween {
                        target: GuiTweenTarget::Position(joystick.home - button.size() / 2.),
                        duration: 0.3,
                        ease: 20, // CircOut
                    },
                ));
            }
            commands.trigger(Released { entity: owner.0 });
        }
        _ => {}
    }
}

fn returned(
    event: On<GuiTweenComplete>,
    owners: Query<&Owner>,
    mut pages: Query<&mut Joystick>,
    gui: GuiQuery,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    if event.property != GuiTweenProperty::Xy {
        return;
    }
    let Ok(owner) = owners.get(event.entity) else {
        return;
    };
    let Ok(mut joystick) = pages.get_mut(owner.0) else {
        return;
    };
    if joystick.returning && joystick.pointer.is_none() {
        restore(&mut joystick, &gui, &mut commands);
    }
}

fn moving(
    event: On<Moving>,
    pages: Query<&Joystick>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    if let Ok(joystick) = pages.get(event.entity) {
        commands.write(GuiObjectCommand::new(
            joystick.output,
            GuiObjectAction::SetText(event.degrees.to_string()),
        ));
    }
}
fn released(
    event: On<Released>,
    pages: Query<&Joystick>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    if let Ok(joystick) = pages.get(event.entity) {
        commands.write(GuiObjectCommand::new(
            joystick.output,
            GuiObjectAction::SetText(String::new()),
        ));
    }
}
