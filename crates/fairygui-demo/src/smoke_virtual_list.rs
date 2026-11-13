//! Exported 1000-row list, actual picking, bounded retained views and GPU paint.
use super::*;
use fairygui_core::{color::Color as GuiColor, package::ObjectType, widget::GraphData};
use std::fmt::Write;

const COLORS: [GuiColor; 4] = [
    GuiColor::new(0.2, 0.6, 0.8, 1.),
    GuiColor::new(0.8, 0.4, 0.2, 1.),
    GuiColor::new(0.3, 0.7, 0.4, 1.),
    GuiColor::new(0.7, 0.3, 0.7, 1.),
];

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    root: Option<usize>,
    list: Option<GObjectId>,
    views: Vec<(GObjectId, Entity)>,
    cursor: Option<Vec2>,
    clicks: Vec<usize>,
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

fn markers(runtime: &mut GuiRuntime, list: GObjectId, variable: bool) {
    runtime
        .enable_virtual_list(list, move |store, index, row| {
            store.set_controller_page_index(row, "IsRead", i32::from(index % 2 == 0));
            store.set_controller_page_index(row, "c1", i32::from(index % 3 == 0));
            store.set_object_text(row, &format!("{index} Mail title here"));
            if let Some(time) = store.child_by_name(row, "timeText") {
                store.set_object_text(time, "5 Nov 2015 16:24:33");
            }
            if variable {
                let width = store.object(row).unwrap().width();
                store.set_size(row, width, 45. + (index % 3) as f32 * 6.);
            }
            let mark = store
                .child_by_name(row, "__virtual_mark")
                .unwrap_or_else(|| {
                    let mark = store.create(ObjectType::Graph);
                    store.set_name(mark, "__virtual_mark");
                    store.set_parent(mark, Some(row));
                    store.set_xy(mark, 8., 8.);
                    store.set_size(mark, 12., 12.);
                    mark
                });
            store.set_graph(
                mark,
                GraphData {
                    graph_type: 1,
                    fill_color: COLORS[index % 4],
                    ..default()
                },
            );
        })
        .unwrap();
}

fn center(
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

fn capture(
    state: &mut State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    let list = state.list.unwrap();
    let info = runtime.store().virtual_list_info(list).unwrap();
    assert!(info.active_count > 1 && info.active_count < 30, "{info:?}");
    let (node, transform) = nodes.get(runtime.entity_of(list).unwrap()).unwrap();
    let min = transform.transform_point2(node.content_box().min);
    let max = transform.transform_point2(node.content_box().max);
    let mut csv = String::from("row,x,y,r,g,b\n");
    let mut samples = 0;
    for row in runtime.store().list_items(list).unwrap() {
        assert!(runtime.store().is_on_stage(row));
        let view = runtime.entity_of(row).unwrap();
        if let Some((_, old)) = state.views.iter().find(|&&(id, _)| id == row) {
            assert_eq!(*old, view);
        } else {
            state.views.push((row, view));
        }
        let index = runtime.store().virtual_item_index(list, row).unwrap();
        let mark = runtime
            .store()
            .child_by_name(row, "__virtual_mark")
            .unwrap();
        let p = center(runtime, nodes, mark);
        let color = COLORS[index % 4];
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
    assert!(samples >= 2, "{suffix} {info:?}");
    assert!(state.views.len() < 45, "bounded row allocation");
    std::fs::write(format!("target/virtual-list-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-virtual-list{suffix}.png"
        )));
    println!(
        "demo smoke: virtual {suffix}, {info:?}, {samples} GPU samples, {} retained row views",
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
        "virtual window timeout step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    if let Some(menu) = demo.menu.and_then(|i| runtime.root_object(i)) {
        runtime.store_mut().set_visible(menu, false);
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/VirtualList.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    let elapsed = state.frame - state.since;
    let scale = window.scale_factor();
    let root = state.root.and_then(|i| runtime.root_object(i));
    controller.root_owner = root.and_then(|root| runtime.entity_of(root));
    match state.step {
        0 => {
            let index = runtime.create_root(handle, &asset.package, "Main").unwrap();
            center_root(&mut runtime, index, (window.width(), window.height()));
            let root = runtime.root_object(index).unwrap();
            let list = runtime.store().child_by_name(root, "mailList").unwrap();
            markers(&mut runtime, list, false);
            runtime.set_virtual_item_count(list, 1000).unwrap();
            controller.root = Some(root);
            controller.package = Some("VirtualList".into());
            state.root = Some(index);
            state.list = Some(list);
            state.step = 1;
            state.since = state.frame;
        }
        1 if elapsed >= 20 => {
            capture(&mut state, &runtime, &nodes, &mut commands, "");
            state.cursor = Some(
                center(
                    &runtime,
                    &nodes,
                    runtime.store().child_by_name(root.unwrap(), "n6").unwrap(),
                ) / scale,
            );
            state.step = 2;
            state.since = state.frame;
        }
        2 | 5 | 8 | 11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step += 1;
            state.since = state.frame;
        }
        3 | 6 | 9 | 12 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step += 1;
            state.since = state.frame;
        }
        4 if elapsed >= 12 => {
            let list = state.list.unwrap();
            assert_eq!(runtime.store().list_selection(list), Some(500));
            assert!(runtime.store().virtual_item_object(list, 500).is_some());
            capture(&mut state, &runtime, &nodes, &mut commands, "-500");
            let row = runtime.store().virtual_item_object(list, 501).unwrap();
            state.cursor = Some(center(&runtime, &nodes, row) / scale);
            state.step = 5;
            state.since = state.frame;
        }
        7 if elapsed >= 8 => {
            assert_eq!(state.clicks, vec![501]);
            state.cursor = Some(
                center(
                    &runtime,
                    &nodes,
                    runtime.store().child_by_name(root.unwrap(), "n8").unwrap(),
                ) / scale,
            );
            state.step = 8;
            state.since = state.frame;
        }
        10 if elapsed >= 12 => {
            let list = state.list.unwrap();
            assert!(runtime.store().virtual_item_object(list, 999).is_some());
            capture(&mut state, &runtime, &nodes, &mut commands, "-bottom");
            state.cursor = Some(
                center(
                    &runtime,
                    &nodes,
                    runtime.store().child_by_name(root.unwrap(), "n7").unwrap(),
                ) / scale,
            );
            state.step = 11;
            state.since = state.frame;
        }
        13 if elapsed >= 12 => {
            let list = state.list.unwrap();
            assert!(runtime.store().virtual_item_object(list, 0).is_some());
            capture(&mut state, &runtime, &nodes, &mut commands, "-top");
            state.step = 14;
            state.since = state.frame;
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
            markers(&mut runtime, list, true);
            runtime.store_mut().set_size(list, 400., 280.);
            runtime.set_virtual_item_count(list, 17).unwrap();
            window.resolution.set(1040., 680.);
            state.step = 16;
            state.since = state.frame;
        }
        16 if elapsed >= 20 => {
            capture(&mut state, &runtime, &nodes, &mut commands, "-resized");
            let list = state.list.unwrap();
            assert_eq!(runtime.store().list_num_items(list), 17);
            state.step = 17;
            state.since = state.frame;
        }
        17 if elapsed >= 8 => {
            runtime
                .set_virtual_item_count(state.list.unwrap(), 0)
                .unwrap();
            state.step = 18;
            state.since = state.frame;
        }
        18 if elapsed >= 8 => {
            controller.root = None;
            controller.root_owner = None;
            controller.package = None;
            runtime.remove_root(&mut commands, state.root.unwrap());
            state.step = 19;
            state.since = state.frame;
        }
        19 if elapsed >= 5 => {
            assert!(
                state
                    .views
                    .iter()
                    .all(|&(id, e)| !runtime.store().contains(id)
                        && runtime.entity_of(id).is_none()
                        && nodes.get(e).is_err())
            );
            println!(
                "demo smoke PASSED: native exported 1000 logical rows, bounded visible construction, select/reveal500, logical click501, bottom/top, retained views, empty FIFO pool, variable heights, actual window resize and disposal"
            );
            exit.write(AppExit::Success);
            state.step = 20;
        }
        _ => {}
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
