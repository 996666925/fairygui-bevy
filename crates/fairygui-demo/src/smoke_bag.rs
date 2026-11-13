//! Actual bag-button/row/close input, pagination, external images and ownership.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    page: Option<Entity>,
    owner: Option<Entity>,
    view: Option<Entity>,
    list: Option<Entity>,
    rows: Vec<Entity>,
    cursor: Option<Vec2>,
    selected_icon: String,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
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
    demo: Res<Demo>,
    gui: GuiQuery,
    bag_windows: Query<(Entity, &GuiWindowState), With<crate::bag_demo::BagWindow>>,
    loaders: Query<&GuiLoaderState>,
    existing: Query<()>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 45.,
        "bag smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let mut next = false;
    match state.step {
        0 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            let page = commands
                .spawn(FairyGuiRoot::new(assets.load("ui/Bag.fui"), "Main"))
                .id();
            commands.write_message(GuiObjectCommand::new(
                page,
                GuiObjectAction::SetPosition(Vec2::new(40., 20.)),
            ));
            state.page = Some(page);
            next = true;
        }
        1 if elapsed >= 16 => {
            let Ok((owner, window)) = bag_windows.single() else {
                return;
            };
            assert!(!window.is_showing());
            state.owner = Some(owner);
            state.view = Some(window.view());
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "bagBtn").unwrap(),
            ));
            next = true;
        }
        2 | 5 | 9 | 13 | 16 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 6 | 10 | 14 | 17 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 16 => {
            let owner = state.owner.unwrap();
            let window = bag_windows.get(owner).unwrap().1;
            assert!(window.is_showing());
            let list = gui.child_by_name(owner, "list").unwrap();
            let content = gui.list(list).unwrap();
            assert_eq!(content.item_count(), 45);
            assert!(content.items().len() < 45, "bag inventory must be virtual");
            state.list = Some(list);
            state.rows = content.items().to_vec();
            let row = content.items()[2];
            state.selected_icon = gui.icon(row).unwrap().to_owned();
            assert!(state.selected_icon.starts_with("icons/i"));
            assert!(gui.text(row).unwrap().parse::<u32>().unwrap() < 100);
            state.cursor = Some(center(row));
            next = true;
        }
        7 if elapsed >= 20 => {
            let owner = state.owner.unwrap();
            let icon = gui.child_by_name(owner, "n11").unwrap();
            let label = gui.child_by_name(owner, "n13").unwrap();
            assert_eq!(gui.icon(icon), Some(state.selected_icon.as_str()));
            assert_eq!(gui.text(label), Some(state.selected_icon.as_str()));
            assert_eq!(
                loaders.get(icon),
                Ok(&GuiLoaderState::Ready),
                "selected inventory PNG must be decoded"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-bag.png"));
            let list = state.list.unwrap();
            let size = gui.scroll(list).unwrap().view_size();
            commands.write_message(GuiObjectCommand::new(
                list,
                GuiObjectAction::ScrollTo {
                    position: Vec2::new(size.x * 2., 0.),
                    animated: false,
                },
            ));
            next = true;
        }
        8 if elapsed >= 16 => {
            let owner = state.owner.unwrap();
            let list = state.list.unwrap();
            assert_eq!(gui.controller(owner, "page").unwrap().selected_index(), 2);
            let content = gui.list(list).unwrap();
            let row = content.items()[2];
            state.rows.extend_from_slice(content.items());
            state.selected_icon = gui.icon(row).unwrap().to_owned();
            state.cursor = Some(center(row));
            next = true;
        }
        11 if elapsed >= 20 => {
            let owner = state.owner.unwrap();
            let icon = gui.child_by_name(owner, "n11").unwrap();
            assert_eq!(gui.icon(icon), Some(state.selected_icon.as_str()));
            let window = bag_windows.get(owner).unwrap().1;
            state.cursor = Some(center(window.close_button().unwrap()));
            next = true;
        }
        12 if elapsed >= 4 => {
            next = true;
        }
        15 if elapsed >= 10 => {
            assert!(
                !bag_windows
                    .get(state.owner.unwrap())
                    .unwrap()
                    .1
                    .is_showing()
            );
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "bagBtn").unwrap(),
            ));
            next = true;
        }
        18 if elapsed >= 16 => {
            let window = bag_windows.get(state.owner.unwrap()).unwrap().1;
            assert!(window.is_showing());
            assert_eq!(Some(window.view()), state.view);
            assert_eq!(gui.list(state.list.unwrap()).unwrap().item_count(), 45);
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        19 if elapsed >= 10 => {
            assert!(!existing.contains(state.owner.unwrap()));
            assert!(!existing.contains(state.view.unwrap()));
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: ECS bag button, retained window, 45 virtual inventory items, decoded external icons, typed row observer preview, page controller, close/reopen and owned disposal"
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
