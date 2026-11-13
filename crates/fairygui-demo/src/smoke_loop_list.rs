//! Actual carousel picking, loop-origin compensation, paint and retained views.
use super::*;
use fairygui_core::{
    color::Color as GuiColor,
    package::ObjectType,
    widget::{ControlData, GraphData},
};
use std::fmt::Write;

const COLORS: [GuiColor; 5] = [
    GuiColor::new(0.2, 0.6, 0.8, 1.),
    GuiColor::new(0.8, 0.4, 0.2, 1.),
    GuiColor::new(0.3, 0.7, 0.4, 1.),
    GuiColor::new(0.7, 0.3, 0.7, 1.),
    GuiColor::new(0.2, 0.75, 0.75, 1.),
];
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    root: Option<usize>,
    initialized: bool,
    list: Option<GObjectId>,
    views: Vec<(GObjectId, Entity)>,
    cursor: Option<Vec2>,
    clicks: Vec<usize>,
    expected: usize,
    wrapped: f32,
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
fn marks(runtime: &mut GuiRuntime, list: GObjectId) {
    for row in runtime.store().list_items(list).unwrap_or_default() {
        let index = runtime.store().virtual_item_index(list, row).unwrap();
        let size = runtime.store().object(row).unwrap().size();
        let mark = runtime
            .store()
            .child_by_name(row, "__loop_mark")
            .unwrap_or_else(|| {
                let mark = runtime.store_mut().create(ObjectType::Graph);
                runtime.store_mut().set_name(mark, "__loop_mark");
                runtime.store_mut().set_parent(mark, Some(row));
                runtime.store_mut().set_size(mark, 12., 12.);
                runtime
                    .store_mut()
                    .set_xy(mark, size.width / 2. - 6., size.height / 2. - 6.);
                runtime.queue_subtree(mark);
                mark
            });
        runtime.store_mut().set_graph(
            mark,
            GraphData {
                graph_type: 1,
                fill_color: COLORS[index],
                ..default()
            },
        );
    }
}
fn capture(
    state: &mut State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    let list = state.list.unwrap();
    let info = runtime.store().virtual_list_info(list).unwrap();
    assert_eq!(
        (info.item_count, info.real_item_count, info.looping),
        (5, 30, true)
    );
    assert!(info.active_count > 1 && info.active_count < 20);
    let (node, t) = nodes.get(runtime.entity_of(list).unwrap()).unwrap();
    let min = t.transform_point2(node.content_box().min);
    let max = t.transform_point2(node.content_box().max);
    let mut csv = String::from("row,x,y,r,g,b\n");
    let mut samples = 0;
    let mut maximum_scale = 1_f32;
    for row in runtime.store().list_items(list).unwrap() {
        let index = runtime.store().virtual_item_index(list, row).unwrap();
        assert!(index < 5);
        let view = runtime.entity_of(row).unwrap();
        if let Some((_, old)) = state.views.iter().find(|&&(id, _)| id == row) {
            assert_eq!(*old, view);
        } else {
            state.views.push((row, view));
        }
        let o = runtime.store().object(row).unwrap();
        maximum_scale = maximum_scale.max(o.scale_x());
        assert!(o.scale_x() >= 1. && o.scale_x() <= 1.2401);
        let ControlData::Button(button) = o.control() else {
            unreachable!()
        };
        assert_eq!(
            button.icon,
            runtime
                .registry()
                .item_url("LoopList", &format!("n{}", index + 1))
        );
        let mark = runtime.store().child_by_name(row, "__loop_mark").unwrap();
        let p = point(runtime, nodes, mark);
        let color = COLORS[index];
        if p.x > min.x + 2. && p.x < max.x - 2. && p.y > min.y + 2. && p.y < max.y - 2. {
            writeln!(
                csv,
                "{index},{},{},{},{},{}",
                p.x, p.y, color.r, color.g, color.b
            )
            .unwrap();
            samples += 1;
        }
    }
    assert!(maximum_scale > 1.01);
    assert!(samples >= 2, "{suffix} {info:?}");
    assert!(state.views.len() < 20);
    let first = runtime.store().list_first_item_in_view(list).unwrap();
    let label = runtime
        .store()
        .child_by_name(runtime.root_object(state.root.unwrap()).unwrap(), "n3")
        .unwrap();
    assert_eq!(
        runtime
            .store()
            .object(label)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        ((first + 1) % 5).to_string()
    );
    std::fs::write(format!("target/loop-list-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-loop-list{suffix}.png"
        )));
    println!(
        "demo smoke: loop {suffix}, {info:?}, {samples} GPU samples, {} retained views",
        state.views.len()
    );
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    demo: Res<Demo>,
    mut controller: ResMut<demos::DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
    gui: GuiQuery,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut clicks: MessageReader<GuiListItemClick>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    for click in clicks.read() {
        if Some(click.event.target) == state.list {
            state.clicks.push(click.index);
        }
    }
    assert!(
        time.elapsed_secs_f64() < 45.,
        "loop smoke timeout step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let scale = window.scale_factor();
    if let Some(menu) = demo.menu.and_then(|i| runtime.root_object(i)) {
        runtime.store_mut().set_visible(menu, false);
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/LoopList.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    if !state.initialized
        && let Some(root) = state.root.and_then(|index| runtime.root_object(index))
        && let Some(view) = runtime.entity_of(root)
    {
        state.initialized = loop_list_demo::initialize(&mut commands, &gui, view, &asset.package);
    }
    let elapsed = state.frame - state.since;
    match state.step {
        0 => {
            let index = runtime.create_root(handle, &asset.package, "Main").unwrap();
            center_root(&mut runtime, index, (window.width(), window.height()));
            let root = runtime.root_object(index).unwrap();
            state.root = Some(index);
            state.list = runtime.store().child_by_name(root, "list");
            controller.root = Some(root);
            controller.package = Some("LoopList".into());
            state.step = 1;
            state.since = state.frame;
        }
        1 if elapsed >= 20 => {
            capture(&mut state, &runtime, &nodes, &mut commands, "");
            state.step = 2;
            state.since = state.frame;
        }
        2 if elapsed >= 8 => {
            let list = state.list.unwrap();
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, 0., 0., false);
            let p = runtime.store().scroll_pane(list).unwrap();
            assert!(p.pos().x > 0. && p.pos().x < p.max_scroll_pos().x);
            state.step = 3;
            state.since = state.frame;
        }
        3 if elapsed >= 10 => {
            capture(&mut state, &runtime, &nodes, &mut commands, "-wrapped-left");
            state.step = 4;
            state.since = state.frame;
        }
        4 if elapsed >= 8 => {
            let list = state.list.unwrap();
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, 5., 0., false);
            state.cursor = Some(point(&runtime, &nodes, list) / scale);
            state.step = 5;
            state.since = state.frame;
        }
        5 if elapsed >= 5 => {
            mouse.press(MouseButton::Left);
            state.step = 6;
            state.since = state.frame;
        }
        6 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().x += 120.;
            state.step = 7;
            state.since = state.frame;
        }
        7 if elapsed >= 4 => {
            let p = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert!(
                p.display_pos().x > 200.,
                "drag must recenter: {:?}",
                p.display_pos()
            );
            state.wrapped = p.display_pos().x;
            capture(&mut state, &runtime, &nodes, &mut commands, "-drag-wrap");
            state.step = 8;
            state.since = state.frame;
        }
        8 if elapsed >= 8 => {
            state.cursor.as_mut().unwrap().x += 60.;
            state.step = 9;
            state.since = state.frame;
        }
        9 if elapsed >= 4 => {
            let p = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert!(
                (p.display_pos().x - (state.wrapped - 60.)).abs() < 2.,
                "shifted drag origin {:?}",
                p.display_pos()
            );
            mouse.release(MouseButton::Left);
            state.step = 10;
            state.since = state.frame;
        }
        10 if elapsed >= 8 => {
            let list = state.list.unwrap();
            if !runtime.store().scroll_pane(list).unwrap().is_animating() {
                assert!(state.clicks.is_empty());
                capture(&mut state, &runtime, &nodes, &mut commands, "-released");
                let (node, t) = nodes.get(runtime.entity_of(list).unwrap()).unwrap();
                let min = t.transform_point2(node.content_box().min);
                let max = t.transform_point2(node.content_box().max);
                let row = runtime
                    .store()
                    .list_items(list)
                    .unwrap()
                    .into_iter()
                    .find(|&row| {
                        let p = point(&runtime, &nodes, row);
                        p.x > min.x + 30. && p.x < max.x - 30.
                    })
                    .unwrap();
                state.expected = runtime.store().virtual_item_index(list, row).unwrap();
                state.cursor = Some(point(&runtime, &nodes, row) / scale);
                state.step = 11;
                state.since = state.frame;
            }
        }
        11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 12;
            state.since = state.frame;
        }
        12 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step = 13;
            state.since = state.frame;
        }
        13 if elapsed >= 8 => {
            let list = state.list.unwrap();
            if !runtime.store().scroll_pane(list).unwrap().is_animating() {
                assert_eq!(state.clicks, vec![state.expected]);
                capture(&mut state, &runtime, &nodes, &mut commands, "-clicked");
                state.step = 14;
                state.since = state.frame;
            }
        }
        14 if elapsed >= 8 => {
            runtime
                .set_virtual_item_count(state.list.unwrap(), 0)
                .unwrap();
            state.step = 15;
            state.since = state.frame;
        }
        15 if elapsed >= 8 => {
            for &(id, view) in &state.views {
                assert!(!runtime.store().is_on_stage(id));
                assert_eq!(runtime.entity_of(id), Some(view));
            }
            let list = state.list.unwrap();
            runtime.set_virtual_item_count(list, 5).unwrap();
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, 1e6, 0., false);
            state.step = 16;
            state.since = state.frame;
        }
        16 if elapsed >= 12 => {
            capture(
                &mut state,
                &runtime,
                &nodes,
                &mut commands,
                "-restored-right",
            );
            state.step = 17;
            state.since = state.frame;
        }
        17 if elapsed >= 8 => {
            let list = state.list.unwrap();
            let height = runtime.store().object(list).unwrap().height();
            runtime.store_mut().set_size(list, 560., height);
            window.resolution.set(1160., 680.);
            state.step = 18;
            state.since = state.frame;
        }
        18 if elapsed >= 20 => {
            capture(&mut state, &runtime, &nodes, &mut commands, "-resized");
            state.step = 19;
            state.since = state.frame;
        }
        19 if elapsed >= 8 => {
            controller.root = None;
            controller.package = None;
            runtime.remove_root(&mut commands, state.root.unwrap());
            state.step = 20;
            state.since = state.frame;
        }
        20 if elapsed >= 5 => {
            assert!(
                state
                    .views
                    .iter()
                    .all(|&(id, e)| !runtime.store().contains(id)
                        && runtime.entity_of(id).is_none()
                        && nodes.get(e).is_err())
            );
            println!(
                "demo smoke PASSED: original five-icon LoopList, sixfold records, both command wraps, actual drag recenter and origin compensation, release tween, logical click, native scale/label, retained FIFO pool, resize and disposal"
            );
            exit.write(AppExit::Success);
            state.step = 21;
        }
        _ => {}
    }
    if let Some(list) = state.list
        && runtime.store().is_virtual_list(list)
    {
        marks(&mut runtime, list);
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
