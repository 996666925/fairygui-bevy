//! Public ECS configuration with real pointer clicks and retained button views.
use super::*;

const BASE_ICON: &str = "ui://9leh0eyfrpmb6";
const SELECTED_ICON: &str = "ui://9leh0eyfrpmbh";
#[derive(Component)]
struct Panel;
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    button: Option<Entity>,
    title: Option<Entity>,
    icon: Option<Entity>,
    child: Option<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Events {
    clicks: usize,
    changes: usize,
}
fn observe_click(_: On<GuiClick>, mut events: ResMut<Events>) {
    events.clicks += 1;
}
fn observe_change(_: On<GuiStateChanged>, mut events: ResMut<Events>) {
    events.changes += 1;
}

fn initialize(
    roots: Query<Entity, (With<Panel>, Added<GuiRootView>)>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<&mut GuiTransform>,
        Query<&mut GuiTextFormat>,
    )>,
) {
    for owner in &roots {
        let (title, icon) = {
            let gui = ui.p0();
            (
                gui.child_by_name(owner, "title").unwrap(),
                gui.child_by_name(owner, "icon").unwrap(),
            )
        };
        for (entity, position, size) in [
            (title, Vec2::new(96., 0.), Vec2::new(310., 110.)),
            (icon, Vec2::new(16., 20.), Vec2::new(64., 64.)),
        ] {
            let mut transforms = ui.p1();
            let mut transform = transforms.get_mut(entity).unwrap();
            transform.position = position;
            transform.size = size;
        }
        let mut formats = ui.p2();
        let mut format = formats.get_mut(title).unwrap();
        format.font_size = 30;
        format.align = 1;
        format.vertical_align = 1;
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Events>()
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    events: Res<Events>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    mut packages: ResMut<Assets<fairygui_bevy::FuiAsset>>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<(&mut GuiButtonSettings, &mut GuiButtonSelected, &mut GuiText)>,
        Query<&mut GuiDisplay>,
        Query<&mut GuiTransform>,
    )>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    transforms: Query<&UiTransform>,
    entities: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "button-properties smoke timed out at {}",
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
            ui.p2().get_mut(menu).unwrap().visible = false;
            state.owner = Some(
                commands
                    .spawn((
                        Panel,
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button2")
                            .with_position(Vec2::new(100., 120.))
                            .with_size(Vec2::new(420., 110.)),
                        GuiButtonSettings {
                            mode: ButtonMode::Check,
                            change_state_on_click: false,
                            selected_title: Some("Selected / 已选中".into()),
                            selected_icon: Some(SELECTED_ICON.into()),
                            ..default()
                        },
                        GuiText::new("Base / 普通"),
                        GuiIcon(BASE_ICON.into()),
                    ))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 16 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let (button, title, icon) = {
                let gui = ui.p0();
                let owner = state.owner.unwrap();
                (
                    gui.view(owner).unwrap(),
                    gui.child_by_name(owner, "title").unwrap(),
                    gui.child_by_name(owner, "icon").unwrap(),
                )
            };
            state.button = Some(button);
            state.title = Some(title);
            state.icon = Some(icon);
            state.child = Some(commands.spawn((Node::default(), ChildOf(button))).id());
            commands
                .entity(button)
                .observe(observe_click)
                .observe(observe_change);
            state.cursor = Some(
                nodes.get(button).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor(),
            );
            next = true;
        }
        2 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 2 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 6 => {
            assert_eq!((events.clicks, events.changes), (1, 0));
            assert!(!ui.p1().get(state.button.unwrap()).unwrap().1.0);
            assert_eq!(ui.p0().text(state.title.unwrap()), Some("Base / 普通"));
            next = true;
        }
        5 if elapsed >= 2 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        6 if elapsed >= 2 => {
            // The setting changes on the release frame, before this click routes.
            ui.p1()
                .get_mut(state.button.unwrap())
                .unwrap()
                .0
                .change_state_on_click = true;
            mouse.release(MouseButton::Left);
            next = true;
        }
        7 if elapsed >= 6 => {
            assert_eq!((events.clicks, events.changes), (2, 1));
            assert!(ui.p1().get(state.button.unwrap()).unwrap().1.0);
            assert_eq!(ui.p0().text(state.button.unwrap()), Some("Base / 普通"));
            assert_eq!(
                ui.p0().text(state.title.unwrap()),
                Some("Selected / 已选中")
            );
            assert_eq!(ui.p0().icon(state.icon.unwrap()), Some(SELECTED_ICON));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-properties-selected.png",
                ));
            next = true;
        }
        8 if elapsed >= 6 => {
            let mut edits = ui.p1();
            let (mut settings, _, _) = edits.get_mut(state.button.unwrap()).unwrap();
            settings.selected_title = Some("Updated / 已更新".into());
            settings.selected_icon = None;
            next = true;
        }
        9 if elapsed >= 6 => {
            assert_eq!(ui.p0().text(state.title.unwrap()), Some("Updated / 已更新"));
            assert_eq!(ui.p0().icon(state.icon.unwrap()), Some(BASE_ICON));
            assert_eq!(events.changes, 1);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-properties-updated.png",
                ));
            next = true;
        }
        10 if elapsed >= 6 => {
            ui.p1().get_mut(state.button.unwrap()).unwrap().0.mode = ButtonMode::Common;
            next = true;
        }
        11 if elapsed >= 6 => {
            assert!(!ui.p1().get(state.button.unwrap()).unwrap().1.0);
            assert_eq!(ui.p0().text(state.title.unwrap()), Some("Base / 普通"));
            assert_eq!(events.changes, 1);
            next = true;
        }
        12 if elapsed >= 2 => {
            let mut edits = ui.p1();
            let (mut settings, mut selected, mut text) =
                edits.get_mut(state.button.unwrap()).unwrap();
            settings.mode = ButtonMode::Radio;
            settings.selected_title = None;
            selected.0 = true;
            text.set("Radio / 单选");
            next = true;
        }
        13 if elapsed >= 6 => {
            assert!(ui.p1().get(state.button.unwrap()).unwrap().1.0);
            assert_eq!(events.changes, 1);
            mouse.press(MouseButton::Left);
            next = true;
        }
        14 if elapsed >= 2 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        15 if elapsed >= 6 => {
            assert_eq!((events.clicks, events.changes), (3, 1));
            assert!(ui.p1().get(state.button.unwrap()).unwrap().1.0);
            assert_eq!(ui.p0().text(state.title.unwrap()), Some("Radio / 单选"));
            next = true;
        }
        16 if elapsed >= 2 => {
            {
                let mut edits = ui.p1();
                let (mut settings, _, mut text) = edits.get_mut(state.button.unwrap()).unwrap();
                settings.mode = ButtonMode::Common;
                settings.down_effect = 2;
                settings.down_effect_value = 0.7;
                text.set("Hold / 按住");
            }
            ui.p3().get_mut(state.button.unwrap()).unwrap().scale = Vec2::new(1.3, 0.9);
            next = true;
        }
        17 if elapsed >= 6 => {
            state.cursor = Some(
                nodes
                    .get(state.button.unwrap())
                    .unwrap()
                    .1
                    .transform_point2(Vec2::ZERO)
                    / window.scale_factor(),
            );
            mouse.press(MouseButton::Left);
            next = true;
        }
        18 if elapsed >= 3 => {
            assert!(
                transforms
                    .get(state.button.unwrap())
                    .unwrap()
                    .scale
                    .abs_diff_eq(Vec2::new(0.91, 0.63), 0.001)
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-properties-down.png",
                ));
            next = true;
        }
        19 if elapsed >= 6 => {
            ui.p1()
                .get_mut(state.button.unwrap())
                .unwrap()
                .0
                .down_effect_value = 0.5;
            next = true;
        }
        20 if elapsed >= 3 => {
            assert!(
                transforms
                    .get(state.button.unwrap())
                    .unwrap()
                    .scale
                    .abs_diff_eq(Vec2::new(0.65, 0.45), 0.001)
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-properties-strength.png",
                ));
            next = true;
        }
        21 if elapsed >= 6 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        22 if elapsed >= 6 => {
            assert!(
                transforms
                    .get(state.button.unwrap())
                    .unwrap()
                    .scale
                    .abs_diff_eq(Vec2::new(1.3, 0.9), 0.001)
            );
            assert_eq!((events.clicks, events.changes), (4, 1));
            assert!(entities.contains(state.child.unwrap()));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-properties-restored.png",
                ));
            next = true;
        }
        23 if elapsed >= 6 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        24 if elapsed >= 6 => {
            for entity in [
                state.button.unwrap(),
                state.title.unwrap(),
                state.icon.unwrap(),
                state.child.unwrap(),
            ] {
                assert!(!entities.contains(entity));
            }
            let package = fairygui_bevy::core::FuiPackage::parse(
                "ButtonInstance",
                include_bytes!("../../fairygui-core/tests/fixtures/ButtonInstance.fui"),
            )
            .unwrap();
            let asset = packages.add(fairygui_bevy::FuiAsset {
                package,
                res_key: "ui/ButtonInstance".into(),
            });
            state.owner = Some(
                commands
                    .spawn(FairyGuiRoot::new(asset, "Main").with_position(Vec2::new(100., 100.)))
                    .id(),
            );
            state.cursor = None;
            next = true;
        }
        25 if elapsed >= 12 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let gui = ui.p0();
            let owner = state.owner.unwrap();
            for (name, visible, font, color) in [
                ("common", "COMMON", 24, Color::srgb_u8(51, 153, 204)),
                ("check", "CHECK ON", 26, Color::srgb_u8(204, 102, 51)),
                ("nested", "NESTED ON", 28, Color::srgb_u8(102, 204, 153)),
                ("bound", "BOUND BASE", 22, Color::srgb_u8(204, 153, 51)),
            ] {
                let button = gui.child_by_name(owner, name).unwrap();
                let mut title = gui.child_by_name(button, "title").unwrap();
                if name == "nested" {
                    title = gui.child_by_name(title, "title").unwrap();
                }
                assert_eq!(gui.text(title), Some(visible), "{name}: exported title");
                assert_eq!(
                    gui.text_format(title).unwrap().font_size,
                    font,
                    "{name}: exported font"
                );
                assert_eq!(gui.color(title), Some(color), "{name}: exported color");
                assert!(nodes.get(title).unwrap().0.size().min_element() > 0.);
            }
            state.button = gui.child_by_name(owner, "nested");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-button-instance.png"));
            next = true;
        }
        26 if elapsed >= 6 => {
            let button = state.button.unwrap();
            let mut values = ui.p1();
            let (_, mut selected, mut title) = values.get_mut(button).unwrap();
            selected.0 = false;
            title.set("Runtime nested");
            next = true;
        }
        27 if elapsed >= 8 => {
            let gui = ui.p0();
            let label = gui.child_by_name(state.button.unwrap(), "title").unwrap();
            let title = gui.child_by_name(label, "title").unwrap();
            assert_eq!(gui.text(title), Some("Runtime nested"));
            assert_eq!(gui.text_format(title).unwrap().font_size, 28);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-button-instance-runtime.png",
                ));
            next = true;
        }
        28 if elapsed >= 6 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        29 if elapsed >= 6 => {
            assert!(!entities.contains(state.button.unwrap()));
            println!(
                "demo smoke PASSED: ECS button properties, pointer clicks, mode and effect changes, exported direct/nested title styles, Common selection policy, initial parent controller preservation, retained runtime edits and disposal"
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
