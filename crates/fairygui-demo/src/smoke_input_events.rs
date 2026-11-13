use super::*;
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyCode, KeyboardInput},
};
use bevy::text::EditableText;
use fairygui_core::{FuiPackage, object::EventKind};

#[derive(Resource, Default)]
pub(super) struct Log {
    events: Vec<fairygui_bevy::GuiTextInput>,
    prevent: Option<Entity>,
}
fn observe(
    mut messages: MessageReader<fairygui_bevy::GuiTextInput>,
    mut log: ResMut<Log>,
    mut commands: MessageWriter<GuiInputCommand>,
) {
    for message in messages.read() {
        if message.event.kind == EventKind::Enter && log.prevent == Some(message.entity) {
            commands.write(GuiInputCommand::PreventSubmit(message.entity));
        }
        log.events.push(message.clone());
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Log>()
        .add_systems(Update, observe.in_set(GuiSystems::Respond));
}
#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    labels: Vec<Entity>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 20
    }
}
fn key(
    commands: &mut Commands,
    window: Entity,
    logical_key: Key,
    key_code: KeyCode,
    text: Option<&str>,
) {
    let text = text.map(Into::into);
    commands.queue(move |world: &mut World| {
        world.write_message(KeyboardInput {
            key_code,
            logical_key,
            text,
            state: ButtonState::Pressed,
            repeat: false,
            window,
        });
    });
}
fn ime(commands: &mut Commands, event: bevy::window::Ime) {
    commands.queue(move |world: &mut World| {
        world.write_message(event);
    });
}
fn cursor(window: &mut Window, view: Entity, nodes: &Query<(&ComputedNode, &UiGlobalTransform)>) {
    let (_, transform) = nodes.get(view).unwrap();
    window.set_cursor_position(Some(
        transform.transform_point2(Vec2::ZERO) / window.scale_factor(),
    ));
}
fn kinds(log: &Log, owner: GObjectId) -> Vec<EventKind> {
    log.events
        .iter()
        .filter(|e| e.event.target == owner)
        .map(|e| {
            assert!(!e.event.bubbling);
            e.event.kind.clone()
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    log: &mut Log,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    editors: &Query<&EditableText>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    mouse: &mut ButtonInput<MouseButton>,
    window_entity: Entity,
    window: &mut Window,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            log.events.clear();
            let package = FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let xy = runtime
                .store()
                .object(original)
                .map(|o| Vec2::new(o.x(), o.y()))
                .unwrap();
            let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, xy.x, xy.y);
            for child in runtime.store().object(root).unwrap().children().to_vec() {
                runtime.store_mut().set_visible(child, false);
            }
            runtime.store_mut().set_visible(original, false);
            for i in 0..3 {
                let owner = runtime
                    .store_mut()
                    .create(fairygui_core::package::ObjectType::InputText);
                runtime.store_mut().set_parent(owner, Some(root));
                runtime
                    .store_mut()
                    .set_xy(owner, 90., 200. + i as f32 * 110.);
                runtime.store_mut().set_size(owner, 900., 80.);
                runtime.store_mut().set_object_font_size(owner, 30);
                runtime.store_mut().set_object_single_line(owner, true);
                runtime.store_mut().set_input_prompt(
                    owner,
                    Some(["Click and type", "Read only", "Password input"][i]),
                );
                runtime.store_mut().set_input_editable(owner, i != 1);
                runtime.store_mut().set_input_password(owner, i == 2);
                if i == 1 {
                    runtime
                        .store_mut()
                        .set_object_text(owner, "Read-only value");
                }
                runtime.store_mut().set_control_color(
                    owner,
                    fairygui_core::color::Color::from_rgba8(20, 70, 110, 255),
                    None,
                );
                runtime.queue_subtree(owner);
                state.owners.push(owner);
                state.labels.push(
                    commands
                        .spawn((
                            Text::new(
                                [
                                    "Pointer focus / KeyboardInput / IME / cancellable Enter",
                                    "Read-only focus / keyboard mutations rejected",
                                    "Password / real keyboard input / single-line Enter",
                                ][i],
                            ),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(18.),
                                ..default()
                            },
                            TextColor(Color::BLACK),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(xy.x + 90.),
                                top: Val::Px(xy.y + 170. + i as f32 * 110.),
                                ..default()
                            },
                            GlobalZIndex(652),
                        ))
                        .id(),
                );
            }
            state.root = Some(index);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            state.views = state
                .owners
                .iter()
                .map(|&o| runtime.entity_of(o).unwrap())
                .collect();
            cursor(window, state.views[0], nodes);
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 1 => {
            mouse.release(MouseButton::Left);
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 8 => {
            assert_eq!(runtime.focused_input(), Some(state.owners[0]));
            assert!(window.ime_enabled);
            key(
                commands,
                window_entity,
                Key::Character("Bevy".into()),
                KeyCode::KeyB,
                Some("Bevy"),
            );
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 8 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "Bevy"
            );
            assert_eq!(
                kinds(log, state.owners[0]),
                [EventKind::FocusIn, EventKind::Input]
            );
            ime(
                commands,
                bevy::window::Ime::Preedit {
                    window: window_entity,
                    value: "预编辑".into(),
                    cursor: None,
                },
            );
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            assert!(editors.get(state.views[0]).unwrap().is_composing());
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "Bevy"
            );
            assert_eq!(kinds(log, state.owners[0]).len(), 2);
            ime(
                commands,
                bevy::window::Ime::Commit {
                    window: window_entity,
                    value: "输入".into(),
                },
            );
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 8 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "Bevy输入"
            );
            let (node, transform) = nodes.get(state.views[0]).unwrap();
            let low = transform.transform_point2(node.content_box().min) / window.scale_factor();
            let high = transform.transform_point2(node.content_box().max) / window.scale_factor();
            assert!(window.ime_position.is_finite());
            assert!(window.ime_position.cmpge(low - Vec2::splat(2.)).all());
            assert!(window.ime_position.cmple(high + Vec2::splat(2.)).all());
            assert_eq!(
                kinds(log, state.owners[0]),
                [EventKind::FocusIn, EventKind::Input, EventKind::Input]
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-input-events.png"));
            cursor(window, state.views[1], nodes);
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 10 => {
            mouse.press(MouseButton::Left);
            state.stage = 9;
            state.since = frames;
        }
        9 if frames - state.since >= 1 => {
            mouse.release(MouseButton::Left);
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 8 => {
            assert_eq!(runtime.focused_input(), Some(state.owners[1]));
            assert!(!window.ime_enabled);
            key(
                commands,
                window_entity,
                Key::Character("X".into()),
                KeyCode::KeyX,
                Some("X"),
            );
            cursor(window, state.views[2], nodes);
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 8 => {
            assert_eq!(
                editors.get(state.views[1]).unwrap().value().to_string(),
                "Read-only value"
            );
            mouse.press(MouseButton::Left);
            state.stage = 12;
            state.since = frames;
        }
        12 if frames - state.since >= 1 => {
            mouse.release(MouseButton::Left);
            state.stage = 13;
            state.since = frames;
        }
        13 if frames - state.since >= 8 => {
            assert_eq!(runtime.focused_input(), Some(state.owners[2]));
            assert!(window.ime_enabled);
            key(
                commands,
                window_entity,
                Key::Character("A😀".into()),
                KeyCode::KeyA,
                Some("A😀"),
            );
            state.stage = 14;
            state.since = frames;
        }
        14 if frames - state.since >= 8 => {
            assert_eq!(
                editors.get(state.views[2]).unwrap().value().to_string(),
                "A😀"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-events-updated.png",
                ));
            state.stage = 15;
            state.since = frames;
        }
        15 if frames - state.since >= 10 => {
            key(commands, window_entity, Key::Enter, KeyCode::Enter, None);
            state.stage = 16;
            state.since = frames;
        }
        16 if frames - state.since >= 8 => {
            assert_eq!(
                kinds(log, state.owners[2]),
                [
                    EventKind::FocusIn,
                    EventKind::Input,
                    EventKind::Enter,
                    EventKind::Change,
                    EventKind::FocusOut
                ]
            );
            assert!(runtime.focused_input().is_none());
            assert!(!window.ime_enabled);
            log.prevent = Some(state.views[0]);
            commands.write_message(GuiInputCommand::Focus(state.views[0]));
            state.stage = 17;
            state.since = frames;
        }
        17 if frames - state.since >= 8 => {
            key(commands, window_entity, Key::Enter, KeyCode::Enter, None);
            state.stage = 18;
            state.since = frames;
        }
        18 if frames - state.since >= 8 => {
            assert_eq!(runtime.focused_input(), Some(state.owners[0]));
            assert_eq!(kinds(log, state.owners[0]).last(), Some(&EventKind::Enter));
            log.prevent = None;
            commands.write_message(GuiInputCommand::Blur);
            runtime.remove_root(commands, state.root.unwrap());
            for &label in &state.labels {
                commands.entity(label).despawn();
            }
            runtime.store_mut().set_visible(original, true);
            state.stage = 19;
            state.since = frames;
        }
        19 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| editors.get(e).is_err()));
            assert!(runtime.focused_input().is_none());
            state.stage = 20;
            println!(
                "demo smoke: authoritative pointer focus, actual KeyboardInput/IME message dispatch, target-only INPUT/ENTER/CHANGE/BLUR order, readonly rejection, password typing, cancellable Enter, IME toggle/position and disposal verified"
            );
        }
        _ => {}
    }
}
