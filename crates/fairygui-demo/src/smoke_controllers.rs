//! Writable controller pages with real button input and native gear animation.
use super::*;

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Owner(Entity);
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    view: Option<Entity>,
    button: Option<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Results {
    initialized: bool,
    clicks: Vec<i32>,
}
fn initialize(
    roots: Query<Entity, (With<Panel>, Added<GuiRootView>)>,
    gui: GuiQuery,
    mut results: ResMut<Results>,
) {
    for owner in &roots {
        assert_eq!(gui.controller(owner, "c1").unwrap().selected_index, 1);
        let image = gui.child_by_name(owner, "n8").unwrap();
        assert!(gui.display(image).unwrap().display_visible());
        results.initialized = true;
    }
}
fn clicked(
    event: On<GuiEvent>,
    owners: Query<&Owner>,
    mut controllers: Query<&mut GuiControllers>,
    mut results: ResMut<Results>,
) {
    if event.event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let owner = owners.get(event.entity).unwrap().0;
    let mut states = controllers.get_mut(owner).unwrap();
    let page = states.get("c1").unwrap().selected_index;
    results.clicks.push(page);
    // Input already selected c1; write only c2 and preserve c1's native change.
    states.get_mut("c2").unwrap().selected_index = page;
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Results>()
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    results: Res<Results>,
    demo: Res<Demo>,
    mut ui: ParamSet<(GuiQuery, Query<&mut GuiControllers>)>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut objects: GuiMut,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "controller smoke timed out at step {}",
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
            let Some(menu) = demo
                .menu_owner
                .filter(|&owner| ui.p0().view(owner).is_some())
            else {
                return;
            };
            objects.get_mut(menu).unwrap().set_visible(false);
            state.owner = Some(
                commands
                    .spawn((
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Controller")
                            .with_position(Vec2::new(40., 60.)),
                        Panel,
                        GuiControllers::new([("c1", 1)]),
                    ))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 30 && results.initialized => {
            let (view, button, graph) = {
                let gui = ui.p0();
                let owner = state.owner.unwrap();
                (
                    gui.view(owner).unwrap(),
                    gui.child_by_name(owner, "n11").unwrap(),
                    gui.child_by_name(owner, "n3").unwrap(),
                )
            };
            assert_eq!(
                ui.p0().transform(graph).unwrap().position(),
                Vec2::new(60., 328.)
            );
            state.view = Some(view);
            state.button = Some(button);
            commands.entity(button).insert(Owner(view)).observe(clicked);
            ui.p1()
                .get_mut(view)
                .unwrap()
                .get_mut("c1")
                .unwrap()
                .selected_index = 0;
            next = true;
        }
        2 if elapsed >= 30 => {
            let gui = ui.p0();
            let owner = state.owner.unwrap();
            let image = gui.child_by_name(owner, "n8").unwrap();
            assert!(!gui.display(image).unwrap().display_visible());
            assert_eq!(gui.controller(owner, "c1").unwrap().selected_index, 0);
            state.cursor = Some(
                nodes
                    .get(state.button.unwrap())
                    .unwrap()
                    .1
                    .transform_point2(Vec2::ZERO)
                    / window.scale_factor(),
            );
            next = true;
        }
        3 | 8 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 | 9 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 30 => {
            assert_eq!(results.clicks, [1]);
            let gui = ui.p0();
            let owner = state.owner.unwrap();
            assert_eq!(gui.controller(owner, "c2").unwrap().selected_index, 1);
            let graph = gui.child_by_name(owner, "n3").unwrap();
            assert_eq!(
                gui.transform(graph).unwrap().position(),
                Vec2::new(60., 328.)
            );
            let ellipse = gui.child_by_name(owner, "n18").unwrap();
            assert!(gui.display(ellipse).unwrap().display_visible());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-controller-properties.png",
                ));
            next = true;
        }
        6 if elapsed >= 4 => {
            ui.p1()
                .get_mut(state.view.unwrap())
                .unwrap()
                .get_mut("c1")
                .unwrap()
                .set_selected_page_id("0");
            next = true;
        }
        7 if elapsed >= 30 => {
            assert_eq!(
                ui.p0()
                    .controller(state.owner.unwrap(), "c1")
                    .unwrap()
                    .selected_index,
                0
            );
            next = true;
        }
        10 if elapsed >= 30 => {
            assert_eq!(results.clicks, [1, 1]);
            ui.p1()
                .get_mut(state.view.unwrap())
                .unwrap()
                .get_mut("c1")
                .unwrap()
                .selected_index = i32::MAX;
            next = true;
        }
        11 if elapsed >= 4 => {
            let gui = ui.p0();
            let state_page = gui.controller(state.owner.unwrap(), "c1").unwrap();
            assert_eq!(state_page.selected_index, -1);
            assert_eq!(state_page.previous_index(), 1);
            assert!(state_page.selected_page_id().is_none());
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        12 if elapsed >= 4 => {
            assert!(ui.p0().get(state.view.unwrap()).is_none());
            println!(
                "demo smoke PASSED: delayed controller initial pages, writable ECS selection, native gear tween geometry and visibility, real linked button input, current observer state, same-frame second-controller edits, page IDs, invalid indices and disposal"
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
