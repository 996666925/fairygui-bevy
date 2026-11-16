//! Input settings changed through public ECS components, with real input routing.
use super::*;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyCode, KeyboardInput},
    },
    input_focus::InputFocusSystems,
    text::{EditableText, TextReadWriteMode},
    window::Ime,
};
use fairygui_bevy::runtime::InputPromptOwner;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    input: Option<Entity>,
    prompt: Option<Entity>,
    cursor: Option<Vec2>,
    direct_checked: bool,
    enter_count: usize,
    label: Option<Entity>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_observer(keep_editing)
        .add_systems(Update, check_direct_input.in_set(GuiSystems::Respond))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(InputFocusSystems::Dispatch)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}

fn keep_editing(event: On<GuiEnter>, mut state: ResMut<State>, mut input: GuiInput) {
    if state.input == Some(event.entity) {
        assert!(input.prevent_submit(event.entity));
        state.enter_count += 1;
    }
}

fn check_direct_input(
    mut state: ResMut<State>,
    mut input: GuiInput,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
) {
    if state.step == 16 && !state.direct_checked {
        let entity = state.input.unwrap();
        input.blur();
        assert_eq!(input.focused_entity(), None);
        assert!(input.focus(entity));
        assert_eq!(input.focused_entity(), Some(entity));
        assert!(!input.prevent_submit(entity));
        key(
            &mut commands,
            windows.single().unwrap(),
            Key::Enter,
            KeyCode::Enter,
            None,
        );
        state.direct_checked = true;
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
fn ime(commands: &mut Commands, event: Ime) {
    commands.queue(move |world: &mut World| {
        world.write_message(event);
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    assets: (Res<AssetServer>, ResMut<Assets<FuiAsset>>),
    mut ui: ParamSet<(
        GuiQuery,
        Query<(&mut GuiTextInputSettings, &mut GuiText, &mut GuiTextFormat)>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
    )>,
    editors: Query<(&EditableText, &TextReadWriteMode, &Text)>,
    prompts: Query<(Entity, &InputPromptOwner, &Text, &TextColor)>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    layouts: Query<&bevy::text::TextLayoutInfo>,
    focus: Res<GuiInputState>,
    entities: Query<()>,
    references: Query<&GObjectRef>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let (assets, mut packages) = assets;
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "input-properties smoke timed out at {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
                return;
            };
            ui.p3().get_mut(menu).unwrap().visible = false;
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Text")
                            .with_position(Vec2::new(70., 80.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 16 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let (input, background, children) = {
                let gui = ui.p0();
                (
                    gui.child_by_name(owner, "n22").unwrap(),
                    gui.child_by_name(owner, "n21").unwrap(),
                    gui.get(owner).unwrap().children().to_vec(),
                )
            };
            for child in children {
                ui.p3().get_mut(child).unwrap().visible = child == input || child == background;
            }
            for (view, position, size) in [
                (background, Vec2::new(30., 40.), Vec2::new(640., 100.)),
                (input, Vec2::new(45., 50.), Vec2::new(610., 80.)),
            ] {
                let mut transforms = ui.p2();
                let mut transform = transforms.get_mut(view).unwrap();
                transform.position = position;
                transform.size = size;
            }
            let mut edits = ui.p1();
            let (mut settings, mut text, mut format) = edits.get_mut(input).unwrap();
            *settings = GuiTextInputSettings {
                max_length: 4,
                prompt_text: Some("[b]Digits only / 数字输入[/b]".into()),
                prompt_color: Color::srgb(0.2, 0.6, 1.),
                ..default()
            };
            settings.try_set_restrict(Some("0-9")).unwrap();
            text.set("");
            format.font = None;
            format.font_size = 32;
            format.auto_size = 0;
            format.single_line = true;
            format.vertical_align = 1;
            state.input = Some(input);
            next = true;
        }
        2 if elapsed >= 12 => {
            let input = state.input.unwrap();
            let object = references.get(input).unwrap().0;
            let (entity, _, text, color) = prompts
                .iter()
                .find(|(_, owner, ..)| owner.0 == object)
                .unwrap();
            assert_eq!(text.0, "Digits only / 数字输入");
            assert_eq!(color.0.to_srgba(), Color::srgb(0.2, 0.6, 1.).to_srgba());
            state.prompt = Some(entity);
            let (prompt_node, prompt_transform) = nodes.get(entity).unwrap();
            let prompt_origin = prompt_transform.transform_point2(prompt_node.content_box().min);
            let prompt_center = prompt_origin.y + layouts.get(entity).unwrap().size.y / 2.;
            let input_center = nodes.get(input).unwrap().1.transform_point2(Vec2::ZERO).y;
            assert!(
                (prompt_center - input_center).abs() < 2.,
                "prompt center {prompt_center} != input center {input_center}"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-properties-prompt.png",
                ));
            state.cursor = Some(
                nodes.get(input).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor(),
            );
            next = true;
        }
        3 if elapsed >= 6 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 2 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 6 => {
            assert_eq!(focus.focused_entity(), state.input);
            key(
                &mut commands,
                window_entity,
                Key::Character("a12b345".into()),
                KeyCode::KeyA,
                Some("a12b345"),
            );
            next = true;
        }
        6 if elapsed >= 8 => {
            let input = state.input.unwrap();
            assert_eq!(editors.get(input).unwrap().0.value().to_string(), "12");
            assert_eq!(ui.p0().text(input), Some("12"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-properties-digits.png",
                ));
            next = true;
        }
        7 if elapsed >= 6 => {
            let mut edits = ui.p1();
            let (mut settings, mut text, _) = edits.get_mut(state.input.unwrap()).unwrap();
            settings.password = true;
            settings.max_length = 5;
            settings.prompt_text = None;
            settings.try_set_restrict(None).unwrap();
            text.set("");
            next = true;
        }
        8 if elapsed >= 8 => {
            assert!(!entities.contains(state.prompt.unwrap()));
            ime(
                &mut commands,
                Ime::Preedit {
                    window: window_entity,
                    value: "中文".into(),
                    cursor: Some((6, 6)),
                },
            );
            next = true;
        }
        9 if elapsed >= 6 => {
            let (editor, ..) = editors.get(state.input.unwrap()).unwrap();
            assert!(editor.is_composing());
            assert_eq!(editor.value().to_string(), "");
            ime(
                &mut commands,
                Ime::Commit {
                    window: window_entity,
                    value: "中😀abc".into(),
                },
            );
            next = true;
        }
        10 if elapsed >= 8 => {
            let input = state.input.unwrap();
            let (editor, _, text) = editors.get(input).unwrap();
            assert!(!editor.is_composing());
            assert_eq!(editor.value().to_string(), "中😀ab");
            assert_eq!(ui.p0().text(input), Some("中😀ab"));
            assert_eq!(
                text.0,
                fairygui_core::text::input::password_text("中😀ab", false)
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-properties-password.png",
                ));
            next = true;
        }
        11 if elapsed >= 6 => {
            ui.p1().get_mut(state.input.unwrap()).unwrap().0.editable = false;
            // Same-frame keyboard input must see the new read-only policy.
            key(
                &mut commands,
                window_entity,
                Key::Backspace,
                KeyCode::Backspace,
                None,
            );
            next = true;
        }
        12 if elapsed >= 8 => {
            let input = state.input.unwrap();
            let (editor, mode, _) = editors.get(input).unwrap();
            assert_eq!(editor.value().to_string(), "中😀ab");
            assert_eq!(*mode, TextReadWriteMode::ReadOnly);
            assert_eq!(focus.focused_entity(), Some(input));
            assert!(!window.ime_enabled);
            let mut edits = ui.p1();
            let (mut settings, mut text, _) = edits.get_mut(input).unwrap();
            settings.password = false;
            text.set("Assigned freely / 程序赋值");
            next = true;
        }
        13 if elapsed >= 8 => {
            let (editor, mode, text) = editors.get(state.input.unwrap()).unwrap();
            assert_eq!(editor.value().to_string(), "Assigned freely / 程序赋值");
            assert_eq!(*mode, TextReadWriteMode::ReadOnly);
            assert_eq!(text.0, "Assigned freely / 程序赋值");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-properties-readonly.png",
                ));
            next = true;
        }
        14 if elapsed >= 6 => {
            let mut edits = ui.p1();
            let (mut settings, mut text, _) = edits.get_mut(state.input.unwrap()).unwrap();
            settings.editable = true;
            settings.prompt_text = Some("Ready again / 继续输入".into());
            settings.prompt_color = Color::srgb(0.1, 0.7, 0.3);
            text.set("");
            next = true;
        }
        15 if elapsed >= 8 => {
            assert!(window.ime_enabled);
            assert_eq!(focus.focused_entity(), state.input);
            let object = references.get(state.input.unwrap()).unwrap().0;
            let (entity, _, text, color) = prompts
                .iter()
                .find(|(_, owner, ..)| owner.0 == object)
                .unwrap();
            assert_eq!(text.0, "Ready again / 继续输入");
            assert_eq!(color.0.to_srgba(), Color::srgb(0.1, 0.7, 0.3).to_srgba());
            state.prompt = Some(entity);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-properties-restored.png",
                ));
            next = true;
        }
        16 if elapsed >= 6 => {
            assert!(state.direct_checked);
            assert_eq!(state.enter_count, 1);
            assert_eq!(focus.focused_entity(), state.input);
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        17 if elapsed >= 8 => {
            assert!(!entities.contains(state.input.unwrap()));
            assert!(!entities.contains(state.prompt.unwrap()));
            assert_eq!(focus.focused_entity(), None);
            assert!(!window.ime_enabled);
            let package = packages.add(FuiAsset {
                package: fairygui_bevy::core::FuiPackage::parse(
                    "LabelInput",
                    include_bytes!("../../fairygui-core/tests/fixtures/LabelInput.fui"),
                )
                .unwrap(),
                res_key: "LabelInput".into(),
            });
            state.owner = Some(
                commands
                    .spawn(FairyGuiRoot::new(package, "Main").with_position(Vec2::new(70., 80.)))
                    .id(),
            );
            next = true;
        }
        18 if elapsed >= 12 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let gui = ui.p0();
            for (name, password, max, font, prompt) in [
                ("direct", false, 4, 24, "Enter four digits"),
                ("nested", true, 6, 22, "Password digits"),
                ("preserved", false, 8, 16, "BaseHint"),
                ("cleared", false, -1, 20, ""),
            ] {
                let label = gui.child_by_name(state.owner.unwrap(), name).unwrap();
                let title = gui.child_by_name(label, "title").unwrap();
                let input = if name == "nested" {
                    gui.child_by_name(title, "title").unwrap()
                } else {
                    title
                };
                let settings = gui.input_settings(label).unwrap();
                assert_eq!((settings.password, settings.max_length), (password, max));
                assert_eq!(settings.prompt_text.as_deref(), Some(prompt));
                assert_eq!(gui.text_format(input).unwrap().font_size, font);
                if name == "nested" {
                    state.label = Some(label);
                    state.input = Some(input);
                }
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-label-input-settings.png",
                ));
            next = true;
        }
        19 if elapsed >= 8 => {
            let (node, transform) = nodes.get(state.input.unwrap()).unwrap();
            state.cursor = Some(
                transform.transform_point2(node.content_box().min + Vec2::splat(10.))
                    / window.scale_factor(),
            );
            next = true;
        }
        20 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        21 if elapsed >= 3 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        22 if elapsed >= 6 => {
            assert_eq!(focus.focused_entity(), state.input);
            key(
                &mut commands,
                window_entity,
                Key::Character("a12345678".into()),
                KeyCode::KeyA,
                Some("a12345678"),
            );
            next = true;
        }
        23 if elapsed >= 8 => {
            let (editor, _, text) = editors.get(state.input.unwrap()).unwrap();
            assert_eq!(editor.value().to_string(), "12345");
            assert_eq!(text.0, "●●●●●");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-label-input-password.png",
                ));
            next = true;
        }
        24 if elapsed >= 8 => {
            let mut edits = ui.p1();
            let (mut settings, mut text, _) = edits.get_mut(state.label.unwrap()).unwrap();
            settings.password = false;
            settings.max_length = 3;
            settings.prompt_text = Some("Runtime nested label".into());
            text.set("");
            next = true;
        }
        25 if elapsed >= 8 => {
            let input = state.input.unwrap();
            let settings = ui.p0().input_settings(input).unwrap().clone();
            assert!(!settings.password);
            assert_eq!(settings.max_length, 3);
            assert_eq!(editors.get(input).unwrap().0.value().to_string(), "");
            let object = references.get(input).unwrap().0;
            let (hint, _, text, _) = prompts
                .iter()
                .find(|(_, owner, ..)| owner.0 == object)
                .unwrap();
            assert_eq!(text.0, "Runtime nested label");
            state.prompt = Some(hint);
            key(
                &mut commands,
                window_entity,
                Key::Character("123a45".into()),
                KeyCode::Digit1,
                Some("123a45"),
            );
            next = true;
        }
        26 if elapsed >= 8 => {
            let (editor, _, text) = editors.get(state.input.unwrap()).unwrap();
            assert_eq!(editor.value().to_string(), "123");
            assert_eq!(text.0, "123");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-label-input-runtime.png"));
            next = true;
        }
        27 if elapsed >= 8 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        28 if elapsed >= 8 => {
            assert!(!entities.contains(state.input.unwrap()));
            assert!(!entities.contains(state.label.unwrap()));
            assert!(!entities.contains(state.prompt.unwrap()));
            assert_eq!(focus.focused_entity(), None);
            assert!(!window.ime_enabled);
            println!(
                "demo smoke PASSED: ECS input properties, keyboard and IME, serialized direct and nested label settings, exported prompt/font/color/restriction/password, retained runtime overrides and disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
