//! Real window checks for model/display scroll separation and clipped row paint.
use super::*;
use fairygui_core::{
    color::Color as GuiColor,
    package::{LoadContext, ObjectType},
    widget::{ControlData, ListLayoutType, ListSelectionMode},
};

const COLORS: [GuiColor; 4] = [
    GuiColor::new(0.2, 0.6, 0.8, 1.),
    GuiColor::new(0.8, 0.4, 0.2, 1.),
    GuiColor::new(0.3, 0.7, 0.4, 1.),
    GuiColor::new(0.7, 0.3, 0.7, 1.),
];

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    list: Option<GObjectId>,
    rows: Vec<GObjectId>,
    views: Vec<Entity>,
    first_center: Vec2,
    start: f32,
    saw_motion: bool,
    label: Option<Entity>,
}

impl State {
    pub fn complete(&self) -> bool {
        self.stage == 14
    }
}

#[derive(Resource, Default)]
struct Focused {
    state: State,
    asset: Option<Handle<FuiAsset>>,
    original: Option<GObjectId>,
    frames: u32,
    cursor: Option<Vec2>,
}

pub(super) fn install_focused(app: &mut App) {
    app.init_resource::<Focused>().add_systems(
        PreUpdate,
        focused
            .after(InputSystems)
            .before(bevy::input_focus::InputFocusSystems::Dispatch)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}

#[allow(clippy::too_many_arguments)]
fn focused(
    mut state: ResMut<Focused>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    scrolls: Query<&ScrollPosition>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut pointers: MessageWriter<PointerInput>,
    mut exit: MessageWriter<AppExit>,
) {
    state.frames += 1;
    if state.frames > 900 {
        exit.write(AppExit::error());
        return;
    }
    let handle = state
        .asset
        .get_or_insert_with(|| server.load("ui/Basics.fui"))
        .clone();
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    window.set_cursor_position(state.cursor);
    if state.original.is_none() {
        let index = runtime
            .create_root(handle, &asset.package, "Demo_List")
            .unwrap();
        let original = runtime.root_object(index).unwrap();
        runtime.store_mut().set_xy(original, 70., 70.);
        state.original = Some(original);
        return;
    }
    let original = state.original.unwrap();
    if runtime.entity_of(original).is_none() {
        return;
    }
    let frames = state.frames;
    drive(
        &mut state.state,
        frames,
        &mut runtime,
        &assets,
        &nodes,
        &scrolls,
        &mut window,
        &mut mouse,
        &mut commands,
        original,
    );
    state.cursor = window.cursor_position();
    move_pointer(window_entity, state.cursor, &mut pointers);
    if state.state.complete() {
        println!("demo smoke PASSED: focused scroll-to-view window checks");
        exit.write(AppExit::Success);
    }
}

fn capture(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
    commands: &mut Commands,
) {
    use std::fmt::Write;
    let list = state.list.unwrap();
    let pane = runtime.store().scroll_pane(list).unwrap();
    let (node, transform) = nodes.get(runtime.entity_of(list).unwrap()).unwrap();
    let lo = transform.transform_point2(node.content_box().min);
    let hi = transform.transform_point2(node.content_box().max);
    let mut csv = String::from("panel,band,x,y,r,g,b,a\n");
    let mut samples = 0;
    for (i, &view) in state.views.iter().enumerate() {
        let (node, transform) = nodes.get(view).unwrap();
        let low = transform.transform_point2(-node.size() / 2.);
        let high = transform.transform_point2(node.size() / 2.);
        let top = low.y.max(lo.y) + 5.;
        let bottom = high.y.min(hi.y) - 5.;
        if bottom <= top {
            continue;
        }
        let x = low.x + (high.x - low.x) * 0.75;
        let color = COLORS[i % COLORS.len()];
        for y in [top, (top + bottom) / 2., bottom] {
            writeln!(csv, "{i},row,{x},{y},{},{},{},1", color.r, color.g, color.b).unwrap();
            samples += 1;
        }
    }
    for y in [lo.y - 8., hi.y + 8.] {
        writeln!(csv, "0,clip,{},{y},1,1,1,1", (lo.x + hi.x) / 2.).unwrap();
    }
    assert!(samples >= 6, "at least two visible rows in {suffix}");
    std::fs::write(format!("target/scroll-view-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-scroll-view{suffix}.png"
        )));
    println!(
        "demo smoke: scroll-view capture {suffix} logical={} display={} row_samples={samples}",
        pane.pos().y,
        pane.display_pos().y
    );
}

fn place_pointer(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    window: &mut Window,
) {
    let row = runtime.store().object(state.rows[3]).unwrap();
    let (node, transform) = nodes.get(state.views[3]).unwrap();
    let scale = node.size().y / row.height();
    let local = -node.size() / 2. + Vec2::new(90., 6.) * scale;
    window.set_cursor_position(Some(
        transform.transform_point2(local) / window.scale_factor(),
    ));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &Assets<FuiAsset>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    scrolls: &Query<&ScrollPosition>,
    window: &mut Window,
    mouse: &mut ButtonInput<MouseButton>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let handle = runtime.asset_handle_for(original).unwrap();
            let package = assets.get(&handle).unwrap().package.clone();
            let position = runtime.store().object(original).unwrap().position();
            let root = runtime.create_root(handle, &package, "Demo_List").unwrap();
            let root_object = runtime.root_object(root).unwrap();
            runtime
                .store_mut()
                .set_xy(root_object, position.x, position.y);
            runtime.store_mut().set_visible(original, false);
            state.root = Some(root);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 12 => {
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            let list = runtime.store().object(root).unwrap().children().iter().copied().find(|&id| {
                matches!(runtime.store().object(id).unwrap().control(), ControlData::List(data)
                    if data.layout == ListLayoutType::SingleColumn && data.component.scroll_pane.is_some())
            }).unwrap();
            let handle = runtime.asset_handle_for(root).unwrap();
            let package = assets.get(&handle).unwrap().package.clone();
            for child in runtime.store().object(root).unwrap().children().to_vec() {
                runtime.store_mut().set_visible(child, child == list);
            }
            for child in runtime.store().list_items(list).unwrap() {
                runtime.store_mut().remove(child);
            }
            let background = runtime.store_mut().create(ObjectType::Graph);
            runtime.store_mut().set_parent(background, Some(root));
            runtime.store_mut().set_xy(background, 130., 120.);
            runtime.store_mut().set_size(background, 730., 330.);
            runtime.store_mut().set_graph(
                background,
                fairygui_core::widget::GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::WHITE,
                    ..default()
                },
            );
            runtime.store_mut().add_child_at(root, background, 0);
            runtime.queue_subtree(background);
            runtime.store_mut().set_xy(list, 180., 200.);
            runtime.store_mut().set_size(list, 430., 138.);
            runtime
                .store_mut()
                .set_list_selection_mode(list, ListSelectionMode::None);
            runtime
                .store_mut()
                .set_list_scroll_item_to_view_on_click(list, true);
            for i in 0..8 {
                let row = fairygui_core::widget::create_from_package(
                    runtime.store_mut(),
                    &package,
                    package.item_index_by_name("Checkbox").unwrap(),
                    &LoadContext::none(),
                )
                .unwrap();
                runtime.store_mut().set_parent(row, Some(list));
                runtime.store_mut().set_size(row, 400., 40.);
                runtime
                    .store_mut()
                    .set_object_text(row, &format!("Scroll row {i}"));
                let fill = runtime.store_mut().create(ObjectType::Graph);
                runtime.store_mut().set_parent(fill, Some(row));
                runtime.store_mut().set_size(fill, 400., 40.);
                runtime.store_mut().set_graph(
                    fill,
                    fairygui_core::widget::GraphData {
                        graph_type: 1,
                        fill_color: COLORS[i % COLORS.len()],
                        ..default()
                    },
                );
                runtime.store_mut().set_touchable(fill, false);
                runtime.store_mut().add_child_at(row, fill, 0);
                runtime.queue_subtree(row);
                state.rows.push(row);
            }
            runtime.store_mut().layout_list(list);
            runtime.store_mut().layout_scroll_bars(list);
            runtime.store_mut().set_scroll_pos(list, 0., 0., false);
            let view = runtime.entity_of(root).unwrap();
            state.label = Some(
                commands
                    .spawn((
                        Text::new("Scroll-to-view: animated endpoint, partial-row click, clipping"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(18.),
                            ..default()
                        },
                        TextColor(Color::BLACK),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(150.),
                            top: Val::Px(135.),
                            ..default()
                        },
                        bevy::picking::Pickable::IGNORE,
                        ChildOf(view),
                    ))
                    .id(),
            );
            state.list = Some(list);
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 14 => {
            state.views = state
                .rows
                .iter()
                .map(|&id| runtime.entity_of(id).unwrap())
                .collect();
            state.first_center = nodes
                .get(state.views[0])
                .unwrap()
                .1
                .transform_point2(Vec2::ZERO);
            capture(state, runtime, nodes, "", commands);
            state.stage = 15;
            state.since = frames;
        }
        15 if frames - state.since >= 10 => {
            let list = state.list.unwrap();
            let index = runtime
                .store()
                .list_item_index(list, state.rows[7])
                .unwrap();
            assert!(
                runtime
                    .store_mut()
                    .scroll_list_item_to_view(list, index, true, false)
            );
            let pane = runtime.store().scroll_pane(list).unwrap();
            assert!(pane.pos().y > pane.display_pos().y);
            state.stage = 3;
            state.since = frames;
        }
        3 => {
            let list = state.list.unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            let display = pane.display_pos().y;
            assert!(
                (scrolls.get(runtime.entity_of(list).unwrap()).unwrap().0.y - display).abs() < 0.01
            );
            let transformed = nodes
                .get(state.views[0])
                .unwrap()
                .1
                .transform_point2(Vec2::ZERO);
            let list_node = nodes.get(runtime.entity_of(list).unwrap()).unwrap().0;
            let scale = list_node.size().y / runtime.store().object(list).unwrap().height();
            assert!(
                (transformed.y - (state.first_center.y - display * scale)).abs() < 1.5,
                "scroll projection: row_y={} initial_y={} display={} physical_scale={} actual_scroll={:?} content={:?} target={} transform={:?}",
                transformed.y,
                state.first_center.y,
                display,
                scale,
                list_node.scroll_position,
                list_node.content_size(),
                pane.pos().y,
                nodes.get(runtime.entity_of(list).unwrap()).unwrap().1
            );
            if display > 0. && display < pane.pos().y && !state.saw_motion {
                state.saw_motion = true;
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-scroll-view-midpoint.png",
                    ));
            }
            if !pane.is_animating() && frames - state.since >= 3 {
                assert!(
                    state.saw_motion,
                    "actual window advanced through intermediate offsets"
                );
                state.stage = 4;
                state.since = frames;
            }
        }
        4 if frames - state.since >= 6 => {
            capture(state, runtime, nodes, "-updated", commands);
            state.stage = 16;
            state.since = frames;
        }
        16 if frames - state.since >= 10 => {
            let list = state.list.unwrap();
            let y = runtime.store().object(state.rows[3]).unwrap().y();
            let view = runtime
                .store()
                .scroll_pane(list)
                .unwrap()
                .view_size()
                .height;
            state.start = y - view + 12.;
            runtime
                .store_mut()
                .set_scroll_pos(list, 0., state.start, false);
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 6 => {
            place_pointer(state, runtime, nodes, window);
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 4 => {
            mouse.release(MouseButton::Left);
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 4 => {
            let list = state.list.unwrap();
            let pane = runtime.store().scroll_pane(list).unwrap();
            assert!(
                pane.pos().y > state.start + 10.,
                "partial row click requested reveal"
            );
            assert!(
                runtime.store().list_selections(list).unwrap().is_empty(),
                "None mode still scrolls"
            );
            if !pane.is_animating() {
                state.stage = 9;
                state.since = frames;
            }
        }
        9 if frames - state.since >= 6 => {
            capture(state, runtime, nodes, "-clicked", commands);
            state.stage = 17;
            state.since = frames;
        }
        17 if frames - state.since >= 10 => {
            let list = state.list.unwrap();
            runtime
                .store_mut()
                .set_list_scroll_item_to_view_on_click(list, false);
            runtime
                .store_mut()
                .set_scroll_pos(list, 0., state.start, false);
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 6 => {
            place_pointer(state, runtime, nodes, window);
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage = 12;
            state.since = frames;
        }
        12 if frames - state.since >= 4 => {
            mouse.release(MouseButton::Left);
            state.stage = 13;
            state.since = frames;
        }
        13 if frames - state.since >= 12 => {
            let pane = runtime.store().scroll_pane(state.list.unwrap()).unwrap();
            assert_eq!(
                pane.pos().y,
                state.start,
                "disabled click reveal preserves offset"
            );
            assert!(!pane.is_animating());
            runtime.remove_root(commands, state.root.unwrap());
            // The label is a child of the removed root and is reclaimed with it.
            state.label = None;
            runtime.store_mut().set_visible(original, true);
            state.stage = 14;
            println!(
                "demo smoke: animated scroll-to-view logical/display offsets, retained UI projection, intermediate movement, clipped row paint, partial-row click in None mode and runtime disabling verified"
            );
        }
        _ => {}
    }
}
