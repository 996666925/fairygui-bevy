//! Actual Chat scrollbar skin, UI layout, picking and rendered captures.
use super::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use fairygui_core::{
    color::Color as GuiColor,
    package::ObjectType,
    scroll::{BAR_NAME, GRIP_NAME},
    widget::{ControlData, GraphData},
};

#[derive(Resource, Default)]
struct State {
    frames: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    root: Option<usize>,
    list: Option<GObjectId>,
    bar: Option<GObjectId>,
    rows: Vec<GObjectId>,
    bar_position: Vec2,
    row_position: Vec2,
    offset: f32,
    cursor: Option<Vec2>,
    clicks: Vec<fairygui_bevy::GuiListItemClick>,
    time: f64,
    saw_fade: bool,
    saw_alignment: bool,
    snap_views: Vec<Entity>,
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
    fraction: Vec2,
    scale: f32,
) -> Vec2 {
    let (node, transform) = nodes.get(runtime.entity_of(id).unwrap()).unwrap();
    transform.transform_point2(node.size() * (fraction - Vec2::splat(0.5))) / scale
}

fn capture(commands: &mut Commands, suffix: &str) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-list-bars{suffix}.png"
        )));
}

fn capture_motion(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    use std::fmt::Write;
    let (node, transform) = nodes
        .get(runtime.entity_of(state.list.unwrap()).unwrap())
        .unwrap();
    let min = transform.transform_point2(node.content_box().min);
    let max = transform.transform_point2(node.content_box().max);
    let mut csv = String::from("kind,row,x,y,r,g,b\n");
    let mut samples = 0;
    for (i, &row) in state.rows.iter().enumerate() {
        let (_, transform) = nodes.get(runtime.entity_of(row).unwrap()).unwrap();
        let p = transform.transform_point2(Vec2::ZERO);
        if p.x > min.x + 1. && p.x < max.x - 1. && p.y > min.y + 1. && p.y < max.y - 1. {
            let (r, g, b) = if i % 2 == 0 {
                (0.2, 0.6, 0.8)
            } else {
                (0.8, 0.4, 0.2)
            };
            writeln!(csv, "row,{i},{},{},{r},{g},{b}", p.x, p.y).unwrap();
            samples += 1;
        }
    }
    assert!(samples >= 2);
    for y in [min.y - 4., max.y + 4.] {
        writeln!(csv, "outside,0,{},{y},0,0,0", (min.x + max.x) * 0.5).unwrap();
    }
    std::fs::write(format!("target/list-motion-samples{suffix}.csv"), csv).unwrap();
    capture(commands, suffix);
    println!(
        "demo smoke: scroll motion capture {suffix}, {samples} row samples and two clip exclusions"
    );
}

fn verify_motion(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    scale: f32,
    logical: f32,
    display: f32,
    delta: f32,
) {
    let pane = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
    assert_eq!(pane.pos().y, logical);
    assert_eq!(pane.display_pos().y, display);
    let row = point(runtime, nodes, state.rows[0], Vec2::splat(0.5), scale);
    assert!(
        (row.y - state.row_position.y - delta).abs() <= 1.5,
        "release projection row={row:?} origin={:?} delta={delta}",
        state.row_position
    );
    assert!(
        (point(runtime, nodes, state.bar.unwrap(), Vec2::splat(0.5), scale) - state.bar_position)
            .length()
            < 0.5
    );
    assert_eq!(state.clicks.len(), 1);
    assert!(
        state
            .rows
            .iter()
            .zip(&state.snap_views)
            .all(|(&r, &v)| runtime.entity_of(r) == Some(v))
    );
}

fn send_wheel(
    wheel: &mut MessageWriter<MouseWheel>,
    window: Entity,
    unit: MouseScrollUnit,
    y: f32,
) {
    wheel.write(MouseWheel {
        unit,
        x: 0.,
        y,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    demo: Res<Demo>,
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut pointers: MessageWriter<PointerInput>,
    mut clicks: MessageReader<fairygui_bevy::GuiListItemClick>,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
    input: Res<fairygui_bevy::input::GuiInputState>,
    visible: Query<&Visibility>,
    mut wheel: MessageWriter<MouseWheel>,
) {
    state.frames += 1;
    state.clicks.extend(clicks.read().cloned());
    if time.elapsed_secs_f64() > 45. {
        eprintln!("list scrollbar smoke timed out at step {}", state.step);
        exit.write(AppExit::error());
        return;
    }
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    window.set_cursor_position(state.cursor);
    let scale = window.scale_factor();
    if let Some(menu) = demo.menu.and_then(|index| runtime.root_object(index)) {
        runtime.store_mut().set_visible(menu, false);
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/Chat.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    let elapsed = state.frames - state.since;
    match state.step {
        0 => {
            let root = runtime.create_root(handle, &asset.package, "Main").unwrap();
            let object = runtime.root_object(root).unwrap();
            runtime.store_mut().set_xy(object, 150., 60.);
            state.root = Some(root);
            state.step = 1;
            state.since = state.frames;
        }
        1 if elapsed >= 20 => {
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            let mut descendants = Vec::new();
            runtime
                .store()
                .for_each_descendant(root, &mut |id| descendants.push(id));
            let list = descendants
                .into_iter()
                .find(|&id| {
                    matches!(
                        runtime.store().object(id).unwrap().control(),
                        ControlData::List(_)
                    ) && runtime
                        .store()
                        .scroll_pane(id)
                        .is_some_and(|pane| pane.vt_bar.is_some())
                })
                .unwrap();
            // Keep the exported pane and bar, replace only its content rows.
            for row in runtime.store().list_items(list).unwrap() {
                runtime.store_mut().remove(row);
            }
            runtime.store_mut().set_size(list, 360., 160.);
            runtime
                .store_mut()
                .set_scroll_bar_display(list, fairygui_core::scroll::ScrollBarDisplayType::Visible);
            runtime
                .store_mut()
                .set_list_scroll_item_to_view_on_click(list, false);
            for i in 0..10 {
                let row = runtime.store_mut().create(ObjectType::Graph);
                runtime.store_mut().set_parent(row, Some(list));
                runtime.store_mut().set_size(row, 320., 40.);
                runtime.store_mut().set_graph(
                    row,
                    GraphData {
                        graph_type: 1,
                        fill_color: if i % 2 == 0 {
                            GuiColor::new(0.2, 0.6, 0.8, 1.)
                        } else {
                            GuiColor::new(0.8, 0.4, 0.2, 1.)
                        },
                        ..default()
                    },
                );
                runtime.queue_subtree(row);
                state.rows.push(row);
            }
            runtime.store_mut().layout_list(list);
            runtime.store_mut().layout_scroll_bars(list);
            runtime.store_mut().set_scroll_pos(list, 0., 0., false);
            let bar = runtime.store().scroll_pane(list).unwrap().vt_bar.unwrap();
            runtime.store_mut().add_child_at(list, bar, 0);
            assert_eq!(runtime.store().list_item_count(list), 10);
            state.list = Some(list);
            state.bar = Some(bar);
            state.step = 2;
            state.since = state.frames;
        }
        2 if elapsed >= 12 => {
            state.bar_position = point(
                &runtime,
                &nodes,
                state.bar.unwrap(),
                Vec2::splat(0.5),
                scale,
            );
            state.row_position = point(&runtime, &nodes, state.rows[0], Vec2::splat(0.5), scale);
            capture(&mut commands, "");
            state.step = 3;
            state.since = state.frames;
        }
        3 if elapsed >= 10 => {
            runtime
                .store_mut()
                .set_scroll_pos(state.list.unwrap(), 0., 60., false);
            state.step = 4;
            state.since = state.frames;
        }
        4 if elapsed >= 10 => {
            let bar = state.bar.unwrap();
            assert!(
                (point(&runtime, &nodes, bar, Vec2::splat(0.5), scale) - state.bar_position)
                    .length()
                    < 0.5
            );
            let row = point(&runtime, &nodes, state.rows[0], Vec2::splat(0.5), scale);
            let list_node = nodes
                .get(runtime.entity_of(state.list.unwrap()).unwrap())
                .unwrap()
                .0;
            assert!(
                (list_node.scroll_position.y - 60. * scale).abs() <= 1.5,
                "Bevy rounds scrolling to physical pixels: {list_node:?}"
            );
            assert!(
                (row - state.row_position + Vec2::new(0., list_node.scroll_position.y / scale))
                    .length()
                    < 0.5
            );
            capture(&mut commands, "-scrolled");
            let track = runtime.store().child_by_name(bar, BAR_NAME).unwrap();
            state.cursor = Some(point(&runtime, &nodes, track, Vec2::new(0.5, 0.92), scale));
            state.clicks.clear();
            state.step = 5;
            state.since = state.frames;
        }
        5 if elapsed >= 10 => {
            mouse.press(MouseButton::Left);
            state.step = 6;
            state.since = state.frames;
        }
        6 if elapsed >= 4 => {
            let pos = runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .pos()
                .y;
            assert!(pos > 60., "track acts while the button is still held");
            state.offset = pos;
            mouse.release(MouseButton::Left);
            state.step = 7;
            state.since = state.frames;
        }
        7 if elapsed >= 8 => {
            let pane = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert!(pane.pos().y > 60., "actual track picking scrolls content");
            assert_eq!(pane.pos().y, state.offset, "release must not scroll twice");
            assert!(!pane.is_animating());
            assert!(state.clicks.is_empty(), "bar track is not a row");
            state.offset = pane.pos().y;
            let grip = runtime
                .store()
                .child_by_name(state.bar.unwrap(), GRIP_NAME)
                .unwrap();
            state.cursor = Some(point(&runtime, &nodes, grip, Vec2::splat(0.5), scale));
            state.step = 8;
            state.since = state.frames;
        }
        8 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 9;
            state.since = state.frames;
        }
        9 if elapsed >= 4 => {
            assert_eq!(
                runtime
                    .store()
                    .scroll_pane(state.list.unwrap())
                    .unwrap()
                    .pos()
                    .y,
                state.offset,
                "grip press does not page the content"
            );
            *state.cursor.as_mut().unwrap() -= Vec2::new(0., 18.);
            state.step = 10;
            state.since = state.frames;
        }
        10 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step = 11;
            state.since = state.frames;
        }
        11 if elapsed >= 8 => {
            let pane = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert!(
                pane.pos().y < state.offset - 5.,
                "actual grip drag scrolls upwards"
            );
            assert!(state.clicks.is_empty(), "grip drag emits no row click");
            runtime
                .store_mut()
                .set_scroll_pos(state.list.unwrap(), 0., 60., false);
            state.step = 12;
            state.since = state.frames;
        }
        12 if elapsed >= 8 => {
            state.cursor = Some(point(
                &runtime,
                &nodes,
                state.rows[2],
                Vec2::splat(0.5),
                scale,
            ));
            state.step = 13;
            state.since = state.frames;
        }
        13 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.step = 14;
            state.since = state.frames;
        }
        14 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            state.step = 15;
            state.since = state.frames;
        }
        15 if elapsed >= 8 => {
            assert_eq!(state.clicks.len(), 1);
            let click = &state.clicks[0];
            assert_eq!(
                (click.event.target, click.item, click.index),
                (state.list.unwrap(), state.rows[2], 2)
            );
            assert!(
                (point(
                    &runtime,
                    &nodes,
                    state.bar.unwrap(),
                    Vec2::splat(0.5),
                    scale
                ) - state.bar_position)
                    .length()
                    < 0.5
            );
            capture(&mut commands, "-clicked");
            println!(
                "demo smoke: Chat pane bar fixed during scroll, track action before release with no repeat, grip drag without page step, no false row events, content index 2 with bar at raw index 0 verified"
            );
            state.step = 16;
            state.since = state.frames;
        }
        16 if elapsed >= 12 => {
            let list = state.list.unwrap();
            runtime
                .store_mut()
                .set_scroll_bar_display(list, fairygui_core::scroll::ScrollBarDisplayType::Auto);
            runtime.store_mut().begin_scroll_drag(list);
            runtime.store_mut().end_scroll_drag(list);
            let grip = runtime
                .store()
                .child_by_name(state.bar.unwrap(), GRIP_NAME)
                .unwrap();
            state.cursor = Some(point(&runtime, &nodes, grip, Vec2::splat(0.5), scale));
            state.step = 17;
            state.since = state.frames;
        }
        17 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            state.time = time.elapsed_secs_f64();
            state.step = 18;
        }
        18 if time.elapsed_secs_f64() - state.time > 1.3 => {
            let bar = state.bar.unwrap();
            assert!(
                runtime
                    .store()
                    .scroll_pane(state.list.unwrap())
                    .unwrap()
                    .grip_dragging(true)
            );
            assert!(runtime.store().object(bar).unwrap().visible());
            assert_eq!(runtime.store().object(bar).unwrap().alpha(), 1.);
            assert_ne!(
                *visible.get(runtime.entity_of(bar).unwrap()).unwrap(),
                Visibility::Hidden
            );
            capture(&mut commands, "-held");
            state.step = 19;
            state.since = state.frames;
        }
        19 if elapsed >= 10 => {
            mouse.release(MouseButton::Left);
            state.time = time.elapsed_secs_f64();
            state.step = 20;
        }
        20 => {
            let bar = state.bar.unwrap();
            let object = runtime.store().object(bar).unwrap();
            let alpha = object.alpha();
            if alpha > 0. && alpha < 0.6 && !state.saw_fade {
                state.saw_fade = true;
                capture(&mut commands, "-fading");
            }
            if !object.visible() && time.elapsed_secs_f64() - state.time >= 1.1 {
                assert!(
                    state.saw_fade,
                    "actual frame loop rendered intermediate fade"
                );
                assert_eq!(alpha, 1., "hidden completion restores alpha");
                assert!(
                    !runtime
                        .store()
                        .scroll_pane(state.list.unwrap())
                        .unwrap()
                        .grip_dragging(true)
                );
                assert_eq!(
                    *visible.get(runtime.entity_of(bar).unwrap()).unwrap(),
                    Visibility::Hidden
                );
                capture(&mut commands, "-hidden");
                state.step = 21;
                state.since = state.frames;
            }
        }
        21 if elapsed >= 10 => {
            mouse.press(MouseButton::Left);
            state.step = 22;
            state.since = state.frames;
        }
        22 if elapsed >= 4 => {
            assert!(
                input
                    .captured()
                    .and_then(|id| runtime.store().registered_scroll_bar_of(id))
                    .is_none(),
                "hidden bar cannot capture a stale pointer position"
            );
            mouse.release(MouseButton::Left);
            state.step = 23;
            state.since = state.frames;
        }
        23 if elapsed >= 4 => {
            assert!(runtime.store_mut().scroll_rect_to_view(
                state.list.unwrap(),
                fairygui_core::geom::Rect::new(0., 0., 50., 40.),
                true,
                true
            ));
            state.step = 24;
            state.since = state.frames;
        }
        24 if elapsed >= 4 => {
            let bar = state.bar.unwrap();
            assert!(runtime.store().object(bar).unwrap().visible());
            assert_eq!(runtime.store().object(bar).unwrap().alpha(), 1.);
            assert_ne!(
                *visible.get(runtime.entity_of(bar).unwrap()).unwrap(),
                Visibility::Hidden
            );
            capture(&mut commands, "-restored");
            state.step = 25;
            state.since = state.frames;
        }
        25 if elapsed >= 12 => {
            println!(
                "demo smoke: held Auto grip survives 1.3 seconds, release fades with restored hidden alpha, hidden picking is rejected and animated scroll wakes the retained bar"
            );
            let list = state.list.unwrap();
            runtime.store_mut().set_scroll_step(list, 12.5);
            runtime
                .store_mut()
                .set_scroll_mouse_wheel_enabled(list, true);
            state.cursor = Some(point(
                &runtime,
                &nodes,
                state.rows[2],
                Vec2::splat(0.5),
                scale,
            ));
            state.step = 26;
            state.since = state.frames;
        }
        26 if elapsed >= 4 => {
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Line, -3.);
            state.step = 27;
            state.since = state.frames;
        }
        27 if elapsed >= 4 => {
            let list = state.list.unwrap();
            assert_eq!(
                runtime.store().scroll_pane(list).unwrap().pos().y,
                25.,
                "line magnitude is sign-normalized and wheel uses twice the custom step"
            );
            runtime
                .store_mut()
                .set_scroll_mouse_wheel_enabled(list, false);
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Line, -1.);
            state.step = 28;
            state.since = state.frames;
        }
        28 if elapsed >= 4 => {
            let list = state.list.unwrap();
            assert_eq!(
                runtime.store().scroll_pane(list).unwrap().pos().y,
                25.,
                "disabled wheel must not move"
            );
            runtime
                .store_mut()
                .set_scroll_mouse_wheel_enabled(list, true);
            runtime.store_mut().set_scroll_step(list, 0.);
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Line, -1.);
            state.step = 29;
            state.since = state.frames;
        }
        29 if elapsed >= 4 => {
            assert_eq!(
                runtime
                    .store()
                    .scroll_pane(state.list.unwrap())
                    .unwrap()
                    .pos()
                    .y,
                75.,
                "zero scroll step restores default 50-pixel wheel movement"
            );
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Pixel, -0.5);
            state.step = 30;
            state.since = state.frames;
        }
        30 if elapsed >= 4 => {
            let list = state.list.unwrap();
            assert_eq!(runtime.store().scroll_pane(list).unwrap().pos().y, 75.5);
            assert_eq!(
                runtime.store().scroll_pane(list).unwrap().display_pos().y,
                75.5
            );
            assert_eq!(state.clicks.len(), 1, "wheel never emits a row click");
            let node = nodes.get(runtime.entity_of(list).unwrap()).unwrap().0;
            assert!(
                (node.scroll_position.y - 75.5 * scale).abs() <= 1.5,
                "actual UI scroll projection follows wheel input"
            );
            capture(&mut commands, "-wheel");
            state.step = 31;
            state.since = state.frames;
        }
        31 if elapsed >= 12 => {
            println!(
                "demo smoke: actual wheel hit, custom/default sign-normalized line steps, disabled/re-enabled wheel, precise pixel delta, UI projection and no row click verified"
            );
            let list = state.list.unwrap();
            state.snap_views = state
                .rows
                .iter()
                .map(|&r| runtime.entity_of(r).unwrap())
                .collect();
            runtime.store_mut().set_scroll_snap_to_item(list, true);
            assert_eq!(runtime.store().scroll_pane(list).unwrap().pos().y, 75.5);
            state.offset = 75.5;
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, 0., 55., true);
            assert_eq!(runtime.store().scroll_pane(list).unwrap().pos().y, 40.);
            state.step = 32;
            state.since = state.frames;
        }
        32 if elapsed >= 4 => {
            let list = state.list.unwrap();
            let p = runtime.store().scroll_pane(list).unwrap();
            state.saw_alignment |= p.display_pos().y > 40. && p.display_pos().y < state.offset;
            let node = nodes.get(runtime.entity_of(list).unwrap()).unwrap().0;
            assert!((node.scroll_position.y - p.display_pos().y * scale).abs() <= 1.5);
            if !p.is_animating() {
                assert!(state.saw_alignment);
                assert_eq!(p.display_pos().y, 40.);
                assert!(
                    state
                        .rows
                        .iter()
                        .zip(&state.snap_views)
                        .all(|(&r, &v)| runtime.entity_of(r) == Some(v))
                );
                capture(&mut commands, "-snap");
                state.step = 33;
                state.since = state.frames;
            }
        }
        33 if elapsed >= 12 => {
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Line, -4.);
            state.step = 34;
            state.since = state.frames;
        }
        34 if elapsed >= 6 => {
            let p = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert_eq!(p.pos().y, 80.);
            assert_eq!(p.display_pos().y, 80.);
            send_wheel(&mut wheel, window_entity, MouseScrollUnit::Pixel, -0.25);
            state.step = 35;
            state.since = state.frames;
        }
        35 if elapsed >= 6 => {
            let list = state.list.unwrap();
            let p = runtime.store().scroll_pane(list).unwrap();
            assert_eq!(p.pos().y, 80.25);
            assert_eq!(p.display_pos().y, 80.25);
            assert_eq!(state.clicks.len(), 1);
            runtime.store_mut().set_scroll_snap_to_item(list, false);
            assert_eq!(runtime.store().scroll_pane(list).unwrap().pos().y, 80.25);
            runtime
                .store_mut()
                .set_scroll_pos_animated(list, 0., 55., false);
            state.step = 36;
            state.since = state.frames;
        }
        36 if elapsed >= 12 => {
            let list = state.list.unwrap();
            assert_eq!(runtime.store().scroll_pane(list).unwrap().pos().y, 55.);
            capture(&mut commands, "-unsnapped");
            state.step = 37;
            state.since = state.frames;
        }
        37 if elapsed >= 12 => {
            println!(
                "demo smoke: item-snap animation, fresh geometry excluding bars, line snap, precise Pixel bypass, inert runtime flag changes and retained rows verified"
            );
            let list = state.list.unwrap();
            runtime.store_mut().set_scroll_pos(list, 0., 0., false);
            runtime.store_mut().set_scroll_snap_to_item(list, true);
            state.cursor = Some(point(&runtime, &nodes, list, Vec2::new(0.4, 0.75), scale));
            state.step = 38;
            state.since = state.frames;
        }
        38 if elapsed >= 8 => {
            state.row_position = point(&runtime, &nodes, state.rows[0], Vec2::splat(0.5), scale);
            state.bar_position = point(
                &runtime,
                &nodes,
                state.bar.unwrap(),
                Vec2::splat(0.5),
                scale,
            );
            mouse.press(MouseButton::Left);
            state.step = 39;
            state.since = state.frames;
        }
        39 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().y -= 55.;
            state.time = time.elapsed_secs_f64();
            state.step = 40;
            state.since = state.frames;
        }
        40 if elapsed >= 8 && time.elapsed_secs_f64() - state.time >= 0.4 => {
            verify_motion(&state, &runtime, &nodes, scale, 55., 55., -55.);
            mouse.release(MouseButton::Left);
            state.step = 41;
            state.since = state.frames;
        }
        41 if elapsed >= 8
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_motion(&state, &runtime, &nodes, scale, 40., 40., -40.);
            capture_motion(&state, &runtime, &nodes, &mut commands, "-release-snap");
            state.step = 42;
            state.since = state.frames;
        }
        42 if elapsed >= 8 => {
            let list = state.list.unwrap();
            runtime.store_mut().set_scroll_snap_to_item(list, false);
            runtime.store_mut().set_scroll_pos(list, 0., 0., false);
            state.cursor = Some(point(&runtime, &nodes, list, Vec2::new(0.4, 0.25), scale));
            state.step = 43;
            state.since = state.frames;
        }
        43 if elapsed >= 8 => {
            state.row_position = point(&runtime, &nodes, state.rows[0], Vec2::splat(0.5), scale);
            state.bar_position = point(
                &runtime,
                &nodes,
                state.bar.unwrap(),
                Vec2::splat(0.5),
                scale,
            );
            mouse.press(MouseButton::Left);
            state.step = 44;
            state.since = state.frames;
        }
        44 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().y += 80.;
            state.step = 45;
            state.since = state.frames;
        }
        45 if elapsed >= 6 => {
            verify_motion(&state, &runtime, &nodes, scale, 0., -40., 40.);
            capture_motion(&state, &runtime, &nodes, &mut commands, "-pull-start");
            state.step = 46;
            state.since = state.frames;
        }
        46 if elapsed >= 8 => {
            mouse.release(MouseButton::Left);
            state.step = 47;
            state.since = state.frames;
        }
        47 if elapsed >= 4
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_motion(&state, &runtime, &nodes, scale, 0., 0., 0.);
            capture_motion(&state, &runtime, &nodes, &mut commands, "-bounce-start");
            state.step = 48;
            state.since = state.frames;
        }
        48 if elapsed >= 8 => {
            let list = state.list.unwrap();
            state.offset = runtime
                .store()
                .scroll_pane(list)
                .unwrap()
                .max_scroll_pos()
                .y;
            runtime
                .store_mut()
                .set_scroll_pos(list, 0., state.offset, false);
            state.cursor = Some(point(&runtime, &nodes, list, Vec2::new(0.4, 0.75), scale));
            state.step = 49;
            state.since = state.frames;
        }
        49 if elapsed >= 8 => {
            state.row_position = point(&runtime, &nodes, state.rows[0], Vec2::splat(0.5), scale);
            state.bar_position = point(
                &runtime,
                &nodes,
                state.bar.unwrap(),
                Vec2::splat(0.5),
                scale,
            );
            mouse.press(MouseButton::Left);
            state.step = 50;
            state.since = state.frames;
        }
        50 if elapsed >= 4 => {
            state.cursor.as_mut().unwrap().y -= 80.;
            state.step = 51;
            state.since = state.frames;
        }
        51 if elapsed >= 6 => {
            verify_motion(
                &state,
                &runtime,
                &nodes,
                scale,
                state.offset,
                state.offset + 40.,
                -40.,
            );
            capture_motion(&state, &runtime, &nodes, &mut commands, "-pull-end");
            state.step = 52;
            state.since = state.frames;
        }
        52 if elapsed >= 8 => {
            mouse.release(MouseButton::Left);
            state.step = 53;
            state.since = state.frames;
        }
        53 if elapsed >= 4
            && !runtime
                .store()
                .scroll_pane(state.list.unwrap())
                .unwrap()
                .is_animating() =>
        {
            verify_motion(
                &state,
                &runtime,
                &nodes,
                scale,
                state.offset,
                state.offset,
                0.,
            );
            capture_motion(&state, &runtime, &nodes, &mut commands, "-bounce-end");
            state.step = 54;
            state.since = state.frames;
        }
        54 if elapsed >= 8 => {
            runtime.remove_root(&mut commands, state.root.unwrap());
            state.step = 55;
            state.since = state.frames;
        }
        55 if elapsed >= 4 => {
            assert!(state.snap_views.iter().all(|&e| nodes.get(e).is_err()));
            println!(
                "demo smoke: actual content drag, stationary release item snap, start/end overscroll paint, native return bounce, fixed bars, clipped rows, no false clicks, retained views and disposal verified"
            );
            println!("demo smoke PASSED: list scrollbar window checks");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
