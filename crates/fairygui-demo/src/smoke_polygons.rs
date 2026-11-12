use super::*;
use fairygui_core::{color::Color as GuiColor, package::ObjectType, widget::GraphData};

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    container: Option<GObjectId>,
    polygon: Option<GObjectId>,
    regular: Option<GObjectId>,
    background: Option<GObjectId>,
    polygon_view: Option<Entity>,
    polygon_clicks: usize,
    background_clicks: usize,
    captured: bool,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 12
    }
}

fn observe_polygon_click(event: On<GuiEvent>, mut smoke: ResMut<Smoke>) {
    if event.event().event.kind == fairygui_core::object::EventKind::Click {
        smoke.polygons.polygon_clicks += 1;
    }
}

fn observe_background_click(event: On<GuiEvent>, mut smoke: ResMut<Smoke>) {
    if event.event().event.kind == fairygui_core::object::EventKind::Click {
        smoke.polygons.background_clicks += 1;
    }
}

fn point(
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    id: GObjectId,
    local: Vec2,
    window: &Window,
) -> Vec2 {
    let object = runtime.store().object(id).unwrap();
    let (node, transform) = nodes.get(runtime.entity_of(id).unwrap()).unwrap();
    transform.transform_point2(
        (local / Vec2::new(object.width(), object.height()) - Vec2::splat(0.5)) * node.size(),
    ) / window.scale_factor()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    mouse: &mut ButtonInput<MouseButton>,
    window: &mut Window,
    root: GObjectId,
) {
    match state.stage {
        0 => {
            let store = runtime.store_mut();
            let container = store.create(ObjectType::Component);
            store.set_parent(container, Some(root));
            store.set_xy(container, 250.0, 180.0);
            store.set_size(container, 450.0, 230.0);
            let bg = store.create(ObjectType::Graph);
            store.set_parent(bg, Some(container));
            store.set_xy(bg, -20.0, -20.0);
            store.set_size(bg, 450.0, 250.0);
            store.set_graph(
                bg,
                GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::new(0.92, 0.94, 0.98, 1.0),
                    ..default()
                },
            );
            let polygon = store.create(ObjectType::Graph);
            store.set_parent(polygon, Some(container));
            store.set_size(polygon, 200.0, 180.0);
            store.set_graph(
                polygon,
                GraphData {
                    graph_type: 3,
                    line_size: 7,
                    line_color: GuiColor::new(0.0, 0.15, 0.4, 1.0),
                    fill_color: GuiColor::new(0.0, 0.7, 0.9, 0.85),
                    polygon_points: Some(vec![
                        0.0, 0.0, 200.0, 0.0, 200.0, 60.0, 60.0, 60.0, 60.0, 180.0, 0.0, 180.0,
                    ]),
                    ..default()
                },
            );
            store.set_rotation(polygon, 8.0);
            store.set_scale(polygon, 0.95, 1.05);
            let regular = store.create(ObjectType::Graph);
            store.set_parent(regular, Some(container));
            store.set_xy(regular, 240.0, 15.0);
            store.set_size(regular, 170.0, 150.0);
            store.set_graph(
                regular,
                GraphData {
                    graph_type: 4,
                    sides: 10,
                    start_angle: -90.0,
                    distances: Some(vec![1.0, 0.45, 1.0, 0.45, 1.0, 0.45, 1.0, 0.45, 1.0, 0.45]),
                    line_size: 3,
                    line_color: GuiColor::new(0.7, 0.1, 0.05, 1.0),
                    fill_color: GuiColor::new(1.0, 0.65, 0.05, 1.0),
                    ..default()
                },
            );
            runtime.queue_subtree(container);
            state.container = Some(container);
            state.polygon = Some(polygon);
            state.regular = Some(regular);
            state.background = Some(bg);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 8 => {
            commands
                .entity(runtime.entity_of(state.container.unwrap()).unwrap())
                .insert(GlobalZIndex(500));
            state.polygon_view = runtime.entity_of(state.polygon.unwrap());
            commands
                .entity(state.polygon_view.unwrap())
                .observe(observe_polygon_click);
            commands
                .entity(runtime.entity_of(state.background.unwrap()).unwrap())
                .observe(observe_background_click);
            window.set_cursor_position(Some(point(
                runtime,
                nodes,
                state.polygon.unwrap(),
                Vec2::new(20.0, 110.0),
                window,
            )));
            state.stage = 2;
            state.since = frames;
        }
        2 | 5 if frames - state.since >= 6 => {
            mouse.press(MouseButton::Left);
            state.stage += 1;
            state.since = frames;
        }
        3 | 6 if frames - state.since >= 6 => {
            mouse.release(MouseButton::Left);
            state.stage += 1;
            state.since = frames;
        }
        4 if frames - state.since >= 6 => {
            assert_eq!(
                state.polygon_clicks, 1,
                "transformed concave interior receives click"
            );
            assert_eq!(state.background_clicks, 0);
            window.set_cursor_position(Some(point(
                runtime,
                nodes,
                state.polygon.unwrap(),
                Vec2::new(120.0, 120.0),
                window,
            )));
            state.stage = 5;
            state.since = frames;
        }
        7 if frames - state.since >= 6 => {
            assert_eq!(
                state.polygon_clicks, 1,
                "concave cutout must not hit polygon"
            );
            assert_eq!(
                state.background_clicks, 1,
                "concave cutout passes to background"
            );
            if !state.captured {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-polygons.png"));
                state.captured = true;
                state.since = frames;
                return;
            }
            runtime
                .store_mut()
                .set_size(state.regular.unwrap(), 210.0, 190.0);
            runtime.store_mut().set_alpha(state.container.unwrap(), 0.5);
            runtime.store_mut().set_control_color(
                state.polygon.unwrap(),
                GuiColor::new(0.75, 0.1, 0.85, 1.0),
                None,
            );
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 8 => {
            assert_eq!(
                runtime.entity_of(state.polygon.unwrap()),
                state.polygon_view
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-polygons-updated.png"));
            state.stage = 9;
            state.since = frames;
        }
        9 if frames - state.since >= 8 => {
            runtime.store_mut().set_graph(
                state.polygon.unwrap(),
                GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::new(0.2, 0.8, 0.3, 1.0),
                    ..default()
                },
            );
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 8 => {
            assert_eq!(
                runtime.entity_of(state.polygon.unwrap()),
                state.polygon_view
            );
            runtime.store_mut().remove(state.container.unwrap());
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 8 => {
            assert!(!runtime.store().contains(state.polygon.unwrap()));
            state.stage = 12;
            println!(
                "demo smoke: concave/regular polygons rendered; transformed interior click, cutout pass-through, size/color/ancestor-alpha updates, shape switching and disposal passed"
            );
        }
        _ => {}
    }
}
