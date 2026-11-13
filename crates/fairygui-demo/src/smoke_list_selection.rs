use super::*;
use bevy::input::keyboard::Key;
use fairygui_core::{
    object::EventKind,
    package::LoadContext,
    widget::{ControlData, ListSelectionMode},
};

#[derive(Resource, Default)]
pub(super) struct Log(Vec<(fairygui_bevy::GuiListItemClick, Vec<usize>)>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<Log>()
        .add_systems(Update, observe.in_set(GuiSystems::Respond));
}

fn observe(
    mut events: MessageReader<fairygui_bevy::GuiListItemClick>,
    mut log: ResMut<Log>,
    runtime: NonSend<GuiRuntime>,
) {
    for event in events.read() {
        let selections = runtime.store().list_selections(event.event.target).unwrap();
        log.0.push((event.clone(), selections));
    }
}

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    operation: usize,
    list: Option<GObjectId>,
    rows: Vec<GObjectId>,
    views: Vec<Entity>,
    label: Option<Entity>,
    children: Vec<(GObjectId, bool)>,
}

impl State {
    pub fn complete(&self) -> bool {
        self.stage == 8
    }
}

fn operation(index: usize) -> (ListSelectionMode, usize, bool, bool, Vec<usize>) {
    match index {
        0 => (ListSelectionMode::Multiple, 1, false, false, vec![1]),
        1 => (ListSelectionMode::Multiple, 4, false, true, vec![1, 4]),
        2 => (
            ListSelectionMode::Multiple,
            2,
            true,
            false,
            vec![1, 2, 3, 4],
        ),
        3 => (ListSelectionMode::Multiple, 3, false, false, vec![3]),
        4 => (
            ListSelectionMode::MultipleSingleClick,
            0,
            false,
            false,
            vec![0, 3],
        ),
        5 => (
            ListSelectionMode::MultipleSingleClick,
            3,
            false,
            false,
            vec![0],
        ),
        6 => (ListSelectionMode::None, 7, true, true, vec![0]),
        7 => (ListSelectionMode::Single, 2, false, false, vec![2]),
        _ => unreachable!(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &Assets<FuiAsset>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    window: &mut Window,
    mouse: &mut ButtonInput<MouseButton>,
    commands: &mut Commands,
    log: &mut Log,
    root: GObjectId,
) {
    match state.stage {
        0 => {
            let asset = assets
                .get(&runtime.asset_handle_for(root).unwrap())
                .unwrap();
            let package = asset.package.clone();
            state.children = runtime
                .store()
                .object(root)
                .unwrap()
                .children()
                .iter()
                .map(|&id| (id, runtime.store().object(id).unwrap().visible()))
                .collect();
            for &(id, _) in &state.children {
                runtime.store_mut().set_visible(id, false);
            }
            let list = runtime
                .store_mut()
                .create(fairygui_core::package::ObjectType::List);
            runtime.store_mut().set_parent(list, Some(root));
            runtime.store_mut().set_xy(list, 230., 90.);
            runtime.store_mut().set_size(list, 480., 400.);
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
                    .set_object_text(row, &format!("Selection row {i}"));
                state.rows.push(row);
            }
            runtime.store_mut().layout_list(list);
            runtime.queue_subtree(list);
            let root_view = runtime.entity_of(root).unwrap();
            state.label = Some(
                commands
                    .spawn((
                        Text::new("Multiple selection: click, Ctrl toggle, Shift range"),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(20.),
                            ..default()
                        },
                        TextColor(Color::BLACK),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(120.),
                            top: Val::Px(35.),
                            ..default()
                        },
                        bevy::picking::Pickable::IGNORE,
                        ChildOf(root_view),
                    ))
                    .id(),
            );
            state.list = Some(list);
            state.stage = 1;
            state.since = frames;
            log.0.clear();
        }
        1 if frames - state.since >= 12 => {
            let (mode, index, shift, ctrl, _) = operation(state.operation);
            runtime
                .store_mut()
                .set_list_selection_mode(state.list.unwrap(), mode);
            state.views = state
                .rows
                .iter()
                .map(|&id| runtime.entity_of(id).unwrap())
                .collect();
            let (_, transform) = nodes.get(state.views[index]).unwrap();
            window.set_cursor_position(Some(
                transform.transform_point2(Vec2::ZERO) / window.scale_factor(),
            ));
            commands.queue(move |world: &mut World| {
                let mut keys = world.resource_mut::<ButtonInput<Key>>();
                keys.reset_all();
                if shift {
                    keys.press(Key::Shift);
                }
                if ctrl {
                    keys.press(Key::Control);
                }
            });
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 4 => {
            mouse.press(MouseButton::Left);
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 4 => {
            mouse.release(MouseButton::Left);
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 8 => {
            let (_, index, shift, ctrl, expected) = operation(state.operation);
            let list = state.list.unwrap();
            assert_eq!(
                runtime.store().list_selections(list).unwrap(),
                expected,
                "operation {}",
                state.operation
            );
            assert_eq!(log.0.len(), state.operation + 1);
            let (message, selections) = log.0.last().unwrap();
            assert_eq!(message.event.kind, EventKind::ClickItem);
            assert_eq!(message.event.target, list);
            assert_eq!(Some(message.item), runtime.entity_of(state.rows[index]));
            assert_eq!((message.shift, message.ctrl), (shift, ctrl));
            assert_eq!(
                selections, &expected,
                "selection is committed before Respond"
            );
            for (i, &row) in state.rows.iter().enumerate() {
                assert_eq!(
                    runtime.store().button_selected(row),
                    Some(expected.contains(&i))
                );
                let ControlData::Button(data) = runtime.store().object(row).unwrap().control()
                else {
                    panic!("row")
                };
                let controller = data.component.controller("button").unwrap();
                let page = controller.selected_page_name().unwrap();
                assert_eq!(
                    matches!(page, "down" | "selectedOver"),
                    expected.contains(&i)
                );
            }
            if state.operation == 2 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-list-selection.png"));
            }
            state.operation += 1;
            state.stage = if state.operation == 8 { 5 } else { 1 };
            state.since = frames;
        }
        5 if frames - state.since >= 10 => {
            let list = state.list.unwrap();
            runtime.store_mut().select_all_list_items(list);
            assert_eq!(runtime.store().list_selections(list).unwrap().len(), 8);
            runtime.store_mut().reverse_list_selection(list);
            assert!(runtime.store().list_selections(list).unwrap().is_empty());
            runtime.store_mut().add_list_selection(list, 5);
            commands.queue(|world: &mut World| {
                world.resource_mut::<ButtonInput<Key>>().reset_all();
            });
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-list-selection-updated.png",
                ));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 16 => {
            runtime.store_mut().remove(state.list.unwrap());
            if let Some(label) = state.label.take() {
                commands.entity(label).despawn();
            }
            for &(id, visible) in &state.children {
                runtime.store_mut().set_visible(id, visible);
            }
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 8 => {
            assert!(runtime.store().object(state.list.unwrap()).is_none());
            assert!(state.views.iter().all(|&view| nodes.get(view).is_err()));
            state.stage = 8;
            println!(
                "demo smoke: native list Single/Multiple/MultipleSingleClick/None selection, Ctrl toggle, Shift range, row button pages, target-only item messages before Respond, all/reverse selection and disposal verified"
            );
        }
        _ => {}
    }
}
