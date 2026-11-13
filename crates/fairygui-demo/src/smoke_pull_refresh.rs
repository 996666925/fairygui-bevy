//! Actual exported pull controls, pointer release, demo responses and GPU marks.
use super::*;
use fairygui_core::{color::Color as GuiColor, package::ObjectType, widget::GraphData};

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    root: Option<usize>,
    lists: [Option<GObjectId>; 2],
    parts: [Option<GObjectId>; 2],
    rows: [Option<GObjectId>; 2],
    marks: Vec<(GObjectId, GObjectId, GuiColor)>,
    views: Vec<(GObjectId, Entity)>,
    cursor: Option<Vec2>,
    releases: [usize; 2],
    clicks: usize,
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
    nodes
        .get(runtime.entity_of(id).unwrap())
        .unwrap()
        .1
        .transform_point2(Vec2::ZERO)
}

fn status(runtime: &GuiRuntime, part: GObjectId) -> i32 {
    runtime
        .store()
        .object(part)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller("c1")
        .unwrap()
        .selected_index
}

fn capture(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    use std::fmt::Write;
    let mut csv = String::from("part,x,y,r,g,b\n");
    let mut count = 0;
    for &(mark, list, color) in &state.marks {
        if !runtime.store().is_on_stage(mark) || !runtime.store().display_visible(mark) {
            continue;
        }
        let (node, transform) = nodes.get(runtime.entity_of(list).unwrap()).unwrap();
        let min = transform.transform_point2(node.content_box().min);
        let max = transform.transform_point2(node.content_box().max);
        let p = point(runtime, nodes, mark);
        if p.x > min.x + 1. && p.x < max.x - 1. && p.y > min.y + 1. && p.y < max.y - 1. {
            writeln!(
                csv,
                "{},{},{},{},{},{}",
                count, p.x, p.y, color.r, color.g, color.b
            )
            .unwrap();
            count += 1;
        }
    }
    assert!(count >= 2, "missing pull capture samples {suffix}");
    std::fs::write(format!("target/pull-refresh-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-pull-refresh{suffix}.png"
        )));
    println!("demo smoke: pull refresh capture {suffix}, {count} GPU samples");
}

fn retained(state: &State, runtime: &GuiRuntime) {
    assert!(
        state
            .views
            .iter()
            .all(|&(id, view)| runtime.entity_of(id) == Some(view))
    );
    assert_eq!(state.clicks, 0);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    demo: Res<Demo>,
    mut controller: ResMut<demos::DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut releases: MessageReader<GuiPullRelease>,
    mut clicks: MessageReader<GuiListItemClick>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    for release in releases.read() {
        for i in 0..2 {
            if state.lists[i] == Some(release.event.target) {
                assert!(!release.event.bubbling);
                state.releases[i] += 1;
            }
        }
    }
    for click in clicks.read() {
        if state.lists.contains(&Some(click.event.target)) {
            state.clicks += 1;
        }
    }
    if time.elapsed_secs_f64() > 45. {
        eprintln!("pull-refresh smoke timed out at step {}", state.step);
        exit.write(AppExit::error());
        return;
    }
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    if let Some(menu) = demo.menu.and_then(|i| runtime.root_object(i)) {
        runtime.store_mut().set_visible(menu, false);
    }
    if controller.root_owner.is_none()
        && let Some(root) = state.root.and_then(|index| runtime.root_object(index))
    {
        controller.root_owner = runtime.entity_of(root);
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/PullToRefresh.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    let elapsed = state.frame - state.since;
    let scale = window.scale_factor();
    match state.step {
        0 => {
            let index = runtime.create_root(handle, &asset.package, "Main").unwrap();
            center_root(&mut runtime, index, (window.width(), window.height()));
            controller.root = runtime.root_object(index);
            controller.package = Some("PullToRefresh".into());
            state.root = Some(index);
            state.step = 1;
            state.since = state.frame;
        }
        1 if elapsed >= 20 => {
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            let first = runtime.store().child_by_name(root, "list1").unwrap();
            let second = runtime.store().child_by_name(root, "list2").unwrap();
            let header = runtime.store().scroll_header(first).unwrap();
            let footer = runtime.store().scroll_footer(second).unwrap();
            state.lists = [Some(first), Some(second)];
            state.parts = [Some(header), Some(footer)];
            state.rows = [
                runtime.store().list_item_at(first, 0),
                runtime.store().list_item_at(second, 0),
            ];
            assert_eq!(runtime.store().list_item_count(first), 1);
            assert_eq!(runtime.store().list_item_count(second), 1);
            assert!(!runtime.store().is_on_stage(header));
            assert!(!runtime.store().is_on_stage(footer));
            assert_eq!(status(&runtime, header), 0);
            assert_eq!(status(&runtime, footer), 0);
            let specs = [
                (header, first, GuiColor::new(0.8, 0.4, 0.2, 1.)),
                (footer, second, GuiColor::new(0.3, 0.7, 0.4, 1.)),
                (
                    state.rows[0].unwrap(),
                    first,
                    GuiColor::new(0.2, 0.6, 0.8, 1.),
                ),
                (
                    state.rows[1].unwrap(),
                    second,
                    GuiColor::new(0.2, 0.6, 0.8, 1.),
                ),
            ];
            for (parent, list, color) in specs {
                let mark = runtime.store_mut().create(ObjectType::Graph);
                runtime.store_mut().set_parent(mark, Some(parent));
                runtime.store_mut().set_xy(mark, 8., 8.);
                runtime.store_mut().set_size(mark, 12., 12.);
                runtime.store_mut().set_graph(
                    mark,
                    GraphData {
                        graph_type: 1,
                        fill_color: color,
                        ..default()
                    },
                );
                runtime.queue_subtree(mark);
                state.marks.push((mark, list, color));
            }
            state.step = 2;
            state.since = state.frame;
        }
        2 if elapsed >= 10 => {
            state.views = state
                .parts
                .iter()
                .chain(state.rows.iter())
                .flatten()
                .map(|&id| (id, runtime.entity_of(id).unwrap()))
                .collect();
            state.cursor = Some(point(&runtime, &nodes, state.rows[0].unwrap()) / scale);
            state.step = 3;
            state.since = state.frame;
        }
        3 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 4;
            state.since = state.frame;
        }
        4 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().y += 200.;
            state.step = 5;
            state.since = state.frame;
        }
        5 if elapsed >= 8 => {
            let list = state.lists[0].unwrap();
            let header = state.parts[0].unwrap();
            assert_eq!(
                runtime.store().scroll_pane(list).unwrap().display_pos().y,
                -100.
            );
            assert_eq!(status(&runtime, header), 1);
            assert_eq!(runtime.store().object(header).unwrap().height(), 100.);
            assert!(runtime.store().is_on_stage(header));
            retained(&state, &runtime);
            capture(&state, &runtime, &nodes, &mut commands, "-armed");
            state.step = 6;
            state.since = state.frame;
        }
        6 if elapsed >= 8 => {
            mouse.release(MouseButton::Left);
            state.step = 7;
            state.since = state.frame;
        }
        7 if elapsed >= 4 => {
            let list = state.lists[0].unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            if !pane.is_animating() {
                assert_eq!(pane.display_pos().y, -71.);
                assert_eq!(pane.header_locked_size(), 71.);
                assert_eq!(pane.pos().y, 0.);
                assert_eq!(status(&runtime, state.parts[0].unwrap()), 2);
                assert_eq!(state.releases, [1, 0]);
                retained(&state, &runtime);
                capture(&state, &runtime, &nodes, &mut commands, "-locked");
                state.step = 8;
                state.since = state.frame;
            }
        }
        8 if elapsed >= 8 => {
            let list = state.lists[0].unwrap();
            if status(&runtime, state.parts[0].unwrap()) == 3
                && !runtime.store().scroll_pane(list).unwrap().is_animating()
            {
                assert_eq!(runtime.store().list_item_count(list), 6);
                assert_eq!(
                    runtime.store().scroll_pane(list).unwrap().display_pos().y,
                    -35.
                );
                retained(&state, &runtime);
                capture(&state, &runtime, &nodes, &mut commands, "-complete");
                state.step = 9;
                state.since = state.frame;
            }
        }
        9 if elapsed >= 8 => {
            let list = state.lists[0].unwrap();
            let header = state.parts[0].unwrap();
            if status(&runtime, header) == 0
                && !runtime.store().scroll_pane(list).unwrap().is_animating()
            {
                assert_eq!(
                    runtime.store().scroll_pane(list).unwrap().display_pos().y,
                    0.
                );
                assert!(!runtime.store().is_on_stage(header));
                assert!(!runtime.store().transition_state(header, "t0").unwrap().0);
                retained(&state, &runtime);
                capture(&state, &runtime, &nodes, &mut commands, "-restored");
                state.step = 10;
                state.since = state.frame;
            }
        }
        10 if elapsed >= 8 => {
            state.cursor = Some(point(&runtime, &nodes, state.rows[1].unwrap()) / scale);
            state.step = 11;
            state.since = state.frame;
        }
        11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 12;
            state.since = state.frame;
        }
        12 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().y -= 80.;
            state.step = 13;
            state.since = state.frame;
        }
        13 if elapsed >= 8 => {
            let list = state.lists[1].unwrap();
            assert_eq!(
                runtime.store().scroll_pane(list).unwrap().display_pos().y,
                40.
            );
            assert!(runtime.store().is_on_stage(state.parts[1].unwrap()));
            retained(&state, &runtime);
            capture(&state, &runtime, &nodes, &mut commands, "-footer-armed");
            state.step = 14;
            state.since = state.frame;
        }
        14 if elapsed >= 8 => {
            mouse.release(MouseButton::Left);
            state.step = 15;
            state.since = state.frame;
        }
        15 if elapsed >= 4 => {
            let list = state.lists[1].unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            if !pane.is_animating() {
                assert_eq!(pane.display_pos().y, 0.);
                assert_eq!(pane.footer_locked_size(), 71.);
                assert_eq!(state.releases, [1, 1]);
                let footer = state.parts[1].unwrap();
                assert_eq!(status(&runtime, footer), 1);
                assert_eq!(runtime.store().object(footer).unwrap().height(), 71.);
                retained(&state, &runtime);
                capture(&state, &runtime, &nodes, &mut commands, "-footer-locked");
                state.step = 16;
                state.since = state.frame;
            }
        }
        16 if elapsed >= 8 => {
            let list = state.lists[1].unwrap();
            let footer = state.parts[1].unwrap();
            if status(&runtime, footer) == 0
                && !runtime.store().scroll_pane(list).unwrap().is_animating()
            {
                assert_eq!(runtime.store().list_item_count(list), 6);
                assert!(!runtime.store().is_on_stage(footer));
                retained(&state, &runtime);
                capture(&state, &runtime, &nodes, &mut commands, "-footer-restored");
                state.step = 17;
                state.since = state.frame;
            }
        }
        17 if elapsed >= 8 => {
            controller.root = None;
            controller.root_owner = None;
            controller.package = None;
            runtime.remove_root(&mut commands, state.root.unwrap());
            state.step = 18;
            state.since = state.frame;
        }
        18 if elapsed >= 4 => {
            assert!(
                state
                    .views
                    .iter()
                    .all(|&(id, e)| runtime.entity_of(id).is_none() && nodes.get(e).is_err())
            );
            assert_eq!(state.releases, [1, 1]);
            assert_eq!(state.clicks, 0);
            println!(
                "demo smoke PASSED: exported Header/Footer, actual pull picking, target-only releases, response-time locks, loading/completion/reset pages, asynchronous data growth, both fit-content extents, retained views, stage/transition lifetime and disposal"
            );
            exit.write(AppExit::Success);
            state.step = 19;
        }
        _ => {}
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
