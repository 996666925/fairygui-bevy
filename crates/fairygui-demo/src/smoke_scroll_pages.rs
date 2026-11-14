//! Actual Bag paging buttons, wheel feedback, retained views and GPU samples.
use super::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use fairygui_core::{
    color::Color as GuiColor,
    package::{LoadContext, ObjectType},
    widget::{ControlData, GraphData, create_from_package},
};

const COLORS: [GuiColor; 3] = [
    GuiColor::new(0.2, 0.6, 0.8, 1.),
    GuiColor::new(0.8, 0.4, 0.2, 1.),
    GuiColor::new(0.3, 0.7, 0.4, 1.),
];

#[derive(Resource, Default)]
struct State {
    frames: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    root: Option<usize>,
    owner: Option<GObjectId>,
    list: Option<GObjectId>,
    pager: Option<GObjectId>,
    buttons: Vec<GObjectId>,
    marks: Vec<(GObjectId, GObjectId, usize)>,
    views: Vec<(GObjectId, Entity)>,
    name: String,
    cursor: Option<Vec2>,
    origin: Vec2,
    saw_motion: bool,
    list_clicks: usize,
    release_time: f64,
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

fn point(
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    id: GObjectId,
) -> Vec2 {
    let (_, transform) = nodes.get(runtime.entity_of(id).unwrap()).unwrap();
    transform.transform_point2(Vec2::ZERO)
}

fn verify_page(state: &State, runtime: &GuiRuntime, page: i32) {
    let owner = runtime.store().object(state.owner.unwrap()).unwrap();
    let c = owner
        .control()
        .component()
        .unwrap()
        .controller(&state.name)
        .unwrap();
    assert_eq!(c.selected_index, page);
    assert_eq!(
        runtime.store().scroll_current_page_x(state.list.unwrap()),
        Some(page)
    );
    for (index, &button) in state.buttons.iter().enumerate() {
        assert_eq!(
            runtime.store().button_selected(button),
            Some(index as i32 == page)
        );
    }
    assert!(
        runtime
            .store()
            .list_selections(state.list.unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.list_clicks, 0);
    assert!(
        state
            .views
            .iter()
            .all(|(id, view)| runtime.entity_of(*id) == Some(*view))
    );
}

fn capture(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
    commands: &mut Commands,
) {
    use std::fmt::Write;
    let (node, transform) = nodes
        .get(runtime.entity_of(state.list.unwrap()).unwrap())
        .unwrap();
    let min = transform.transform_point2(node.content_box().min);
    let max = transform.transform_point2(node.content_box().max);
    let mut csv = String::from("row,x,y,r,g,b\n");
    let mut count = 0;
    for (i, &(_, mark, page)) in state.marks.iter().enumerate() {
        let p = point(runtime, nodes, mark);
        if p.x > min.x + 1. && p.x < max.x - 1. && p.y > min.y + 1. && p.y < max.y - 1. {
            let c = COLORS[page % COLORS.len()];
            writeln!(csv, "{i},{},{},{},{},{}", p.x, p.y, c.r, c.g, c.b).unwrap();
            count += 1;
        }
    }
    assert!(count >= 10, "missing visible Bag page samples: {count}");
    std::fs::write(format!("target/scroll-pages-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-scroll-pages{suffix}.png"
        )));
    println!("demo smoke: Bag page capture {suffix}, {count} GPU samples");
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    demo: Res<Demo>,
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    scrolls: Query<&ScrollPosition>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut pointers: MessageWriter<PointerInput>,
    mut wheel: MessageWriter<MouseWheel>,
    mut clicks: MessageReader<GuiListItemClick>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frames += 1;
    let list = state.list;
    state.list_clicks += clicks
        .read()
        .filter(|e| Some(e.event.target) == list)
        .count();
    if time.elapsed_secs_f64() > 45. {
        eprintln!("Bag page smoke timed out at step {}", state.step);
        exit.write(AppExit::error());
        return;
    }
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    window.set_cursor_position(state.cursor);
    if let Some(menu) = demo.menu.and_then(|index| runtime.root_object(index)) {
        runtime.store_mut().set_visible(menu, false);
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/Bag.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    let elapsed = state.frames - state.since;
    match state.step {
        0 => {
            let root = runtime
                .create_root(handle, &asset.package, "BagWin")
                .unwrap();
            let owner = runtime.root_object(root).unwrap();
            let size = runtime.store().object(owner).unwrap().size();
            runtime.store_mut().set_xy(
                owner,
                (window.width() - size.width) * 0.5,
                (window.height() - size.height) * 0.5,
            );
            let list = runtime.store().child_by_name(owner, "list").unwrap();
            let ControlData::List(data) = runtime.store().object(list).unwrap().control() else {
                panic!("Bag list")
            };
            let url = data.default_item.as_ref().unwrap();
            let (package, item) = runtime
                .resolve_url(url)
                .map(|(p, i)| (p.clone(), i))
                .unwrap();
            for i in 0..45 {
                let row =
                    create_from_package(runtime.store_mut(), &package, item, &LoadContext::none())
                        .unwrap();
                runtime.store_mut().set_parent(row, Some(list));
                runtime.store_mut().set_object_text(row, &i.to_string());
                let mark = runtime.store_mut().create(ObjectType::Graph);
                runtime.store_mut().set_parent(mark, Some(row));
                runtime.store_mut().set_xy(mark, 6., 6.);
                runtime.store_mut().set_size(mark, 20., 20.);
                state.marks.push((row, mark, 0));
            }
            let c = &runtime
                .store()
                .object(owner)
                .unwrap()
                .control()
                .component()
                .unwrap()
                .controllers[0];
            state.name = c.name.clone();
            state.pager = runtime.store().child_by_name(owner, "n25");
            state.root = Some(root);
            state.owner = Some(owner);
            state.list = Some(list);
            state.step = 1;
            state.since = state.frames;
        }
        1 if elapsed >= 12 && runtime.entity_of(state.list.unwrap()).is_some() => {
            let list = state.list.unwrap();
            state.buttons = runtime.store().list_items(state.pager.unwrap()).unwrap();
            assert_eq!(state.buttons.len(), 3);
            assert_eq!(
                runtime.store().scroll_page_controller(list),
                Some((state.owner.unwrap(), 0))
            );
            let width = runtime.store().scroll_pane(list).unwrap().view_size().width;
            assert_eq!(
                runtime
                    .store()
                    .scroll_pane(list)
                    .unwrap()
                    .content_size()
                    .width,
                width * 3.
            );
            assert!(
                runtime
                    .store()
                    .scroll_pane(list)
                    .unwrap()
                    .max_scroll_pos()
                    .x
                    >= width * 2.
            );
            for item in &mut state.marks {
                let page = (runtime.store().object(item.0).unwrap().x() / width).floor() as usize;
                item.2 = page;
                runtime.store_mut().set_graph(
                    item.1,
                    GraphData {
                        graph_type: 1,
                        fill_color: COLORS[page % COLORS.len()],
                        ..default()
                    },
                );
            }
            state.views = state
                .marks
                .iter()
                .flat_map(|(row, mark, _)| [*row, *mark])
                .chain([list])
                .map(|id| (id, runtime.entity_of(id).unwrap()))
                .collect();
            state.origin = point(&runtime, &nodes, state.marks[0].1);
            runtime
                .store_mut()
                .set_scroll_mouse_wheel_enabled(list, true);
            state.step = 2;
            state.since = state.frames;
        }
        2 if elapsed >= 12 => {
            verify_page(&state, &runtime, 0);
            capture(&state, &runtime, &nodes, "", &mut commands);
            state.step = 3;
            state.since = state.frames;
        }
        3 if elapsed >= 8 => {
            // Native Bag exports a noninteractive indicator. Opt in only for
            // this test to exercise list selection -> controller -> pane input.
            runtime
                .store_mut()
                .set_touchable(state.pager.unwrap(), true);
            state.cursor = Some(point(&runtime, &nodes, state.buttons[2]) / window.scale_factor());
            state.step = 4;
            state.since = state.frames;
        }
        4 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 5;
            state.since = state.frames;
        }
        5 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step = 6;
            state.since = state.frames;
        }
        6 if elapsed >= 1 => {
            verify_page(&state, &runtime, 2);
            let list = state.list.unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            state.saw_motion |= pane.display_pos().x > 0. && pane.display_pos().x < pane.pos().x;
            let actual = point(&runtime, &nodes, state.marks[0].1);
            assert!(
                (state.origin.x - actual.x - pane.display_pos().x * window.scale_factor()).abs()
                    <= 2.,
                "Bag projection origin={:?} actual={actual:?} model={} scroll={:?} scale={}",
                state.origin,
                pane.display_pos().x,
                scrolls.get(runtime.entity_of(list).unwrap()).unwrap().0,
                window.scale_factor()
            );
            if elapsed >= 4 && !pane.is_animating() {
                assert!(state.saw_motion);
                assert_eq!(
                    scrolls.get(runtime.entity_of(list).unwrap()).unwrap().0.x,
                    pane.display_pos().x
                );
                capture(&state, &runtime, &nodes, "-page2", &mut commands);
                state.step = 7;
                state.since = state.frames;
            }
        }
        7 if elapsed >= 8 => {
            let mark = state
                .marks
                .iter()
                .find(|(_, _, page)| *page == 2)
                .unwrap()
                .1;
            state.cursor = Some(point(&runtime, &nodes, mark) / window.scale_factor());
            state.step = 8;
            state.since = state.frames;
        }
        8 if elapsed >= 4 => {
            wheel.write(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.,
                y: 1.,
                window: window_entity,
                phase: bevy::input::touch::TouchPhase::Moved,
            });
            state.step = 9;
            state.since = state.frames;
        }
        9 if elapsed >= 8 => {
            verify_page(&state, &runtime, 1);
            capture(&state, &runtime, &nodes, "-wheel", &mut commands);
            state.step = 10;
            state.since = state.frames;
        }
        10 if elapsed >= 8 => {
            let list = state.list.unwrap();
            runtime.store_mut().set_scroll_page_controller(list, None);
            runtime
                .store_mut()
                .set_controller_page_index(state.owner.unwrap(), &state.name, 0);
            assert_eq!(runtime.store().scroll_current_page_x(list), Some(1));
            runtime
                .store_mut()
                .set_scroll_page_controller(list, Some((state.owner.unwrap(), 0)));
            assert_eq!(
                runtime.store().scroll_current_page_x(list),
                Some(1),
                "binding alone must remain inert"
            );
            state.cursor = Some(point(&runtime, &nodes, state.buttons[2]) / window.scale_factor());
            state.step = 11;
            state.since = state.frames;
        }
        11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 12;
            state.since = state.frames;
        }
        12 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step = 13;
            state.since = state.frames;
        }
        13 if elapsed >= 8
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_page(&state, &runtime, 2);
            capture(&state, &runtime, &nodes, "-restored", &mut commands);
            state.step = 14;
            state.since = state.frames;
        }
        14 if elapsed >= 8 => {
            let list = state.list.unwrap();
            let w = runtime.store().scroll_pane(list).unwrap().view_size().width;
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, w * 1.31, 0., true);
            verify_page(&state, &runtime, 1);
            state.step = 15;
            state.since = state.frames;
        }
        15 if elapsed >= 8
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_page(&state, &runtime, 1);
            capture(&state, &runtime, &nodes, "-aligned", &mut commands);
            state.step = 16;
            state.since = state.frames;
        }
        16 if elapsed >= 8 => {
            let list = state.list.unwrap();
            let w = runtime.store().scroll_pane(list).unwrap().view_size().width;
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, w * 1.29, 0., false);
            verify_page(&state, &runtime, 1);
            assert!(!runtime.store().scroll_pane(list).unwrap().is_animating());
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, w * 1.31, 0., true);
            verify_page(&state, &runtime, 2);
            state.step = 17;
            state.since = state.frames;
        }
        17 if elapsed >= 8
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_page(&state, &runtime, 2);
            capture(&state, &runtime, &nodes, "-aligned-forward", &mut commands);
            state.step = 18;
            state.since = state.frames;
        }
        18 if elapsed >= 8 => {
            state.cursor =
                Some(point(&runtime, &nodes, state.list.unwrap()) / window.scale_factor());
            state.step = 19;
            state.since = state.frames;
        }
        19 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 20;
            state.since = state.frames;
        }
        20 if elapsed >= 4 => {
            let width = runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .view_size()
                .width;
            state.cursor.as_mut().unwrap().x += width * 0.35;
            state.release_time = time.elapsed_secs_f64();
            state.step = 21;
            state.since = state.frames;
        }
        21 if elapsed >= 8 && time.elapsed_secs_f64() - state.release_time >= 0.4 => {
            let pane = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert!(pane.pos().x < pane.max_scroll_pos().x);
            assert!(pane.pos().x > pane.view_size().width);
            state.saw_motion = false;
            mouse.release(MouseButton::Left);
            state.step = 22;
            state.since = state.frames;
        }
        22 if elapsed >= 1 => {
            let list = state.list.unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            state.saw_motion |= pane.is_animating();
            let actual = point(&runtime, &nodes, state.marks[0].1);
            assert!(
                (state.origin.x - actual.x - pane.display_pos().x * window.scale_factor()).abs()
                    <= 2.
            );
            if elapsed >= 8 && !pane.is_animating() {
                assert!(state.saw_motion);
                verify_page(&state, &runtime, 1);
                capture(&state, &runtime, &nodes, "-release", &mut commands);
                state.step = 23;
                state.since = state.frames;
            }
        }
        23 if elapsed >= 8 => {
            runtime.remove_root(&mut commands, state.root.unwrap());
            state.step = 24;
            state.since = state.frames;
        }
        24 if elapsed >= 4 => {
            assert!(state.views.iter().all(|(id, view)| runtime.entity_of(*id).is_none() && nodes.get(*view).is_err()));
            println!(
                "demo smoke PASSED: Bag pager animation, wheel/controller/indicator feedback, directional fractional page alignment, actual content drag/release page snap, inert unbind/rebind, retained views and disposal"
            );
            exit.write(AppExit::Success);
            state.step = 25;
        }
        _ => {}
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
