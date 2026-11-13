//! Mixed-template virtual rows through entity components and real pointer input.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owner: Option<Entity>,
    list: Option<Entity>,
    rows: Vec<Entity>,
    cursor: Option<Vec2>,
    clicked: Option<usize>,
}
#[derive(Resource, Default)]
struct Events {
    clicks: Vec<(Entity, usize)>,
    swaps: usize,
}
#[derive(Component)]
struct Swap(Entity);
fn clicked(event: On<GuiListItemClick>, mut events: ResMut<Events>) {
    events.clicks.push((event.entity, event.index));
}
fn swapped(
    event: On<GuiEvent>,
    swaps: Query<&Swap>,
    mut lists: Query<&mut GuiVirtualList>,
    mut events: ResMut<Events>,
) {
    if event.event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(swap) = swaps.get(event.entity) else {
        return;
    };
    let Ok(mut list) = lists.get_mut(swap.0) else {
        return;
    };
    events.swaps += 1;
    list.set_item_provider(|index| {
        Some(
            if index % 2 == 0 {
                "chatRight"
            } else {
                "chatLeft"
            }
            .into(),
        )
    });
}
fn content() -> GuiVirtualList {
    GuiVirtualList::new(80, |index, row| {
        if let Some(mut text) = row.child_by_path("msg") {
            text.set_text(&message(index));
            text.ensure_size_correct();
        }
        if let Some(mut name) = row.child_by_path("name") {
            name.set_text("FairyGUI");
        }
        row.set_icon(if index % 2 == 0 {
            "ui://Chat/r1"
        } else {
            "ui://Chat/r0"
        });
    })
    .with_item_provider(|index| {
        Some(
            if index % 2 == 0 {
                "ui://Chat/chatLeft"
            } else {
                "chatRight"
            }
            .into(),
        )
    })
}
fn message(index: usize) -> String {
    match index {
        1 => "Long message with CJK 中文 and wrapping. ".repeat(9),
        2 => "<img src='ui://Chat/1f600'/>".into(),
        _ => format!("Message {index}"),
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Events>()
        .add_systems(Update, bind.in_set(GuiSystems::Initialize))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn bind(
    mut commands: Commands,
    roots: Query<(Entity, &FairyGuiRoot), Added<GuiRootView>>,
    gui: GuiQuery,
) {
    for (root, source) in &roots {
        if !source
            .asset
            .path()
            .is_some_and(|path| path.path().ends_with("Chat.fui"))
        {
            continue;
        }
        let list = gui.child_by_name(root, "list").unwrap();
        commands.entity(list).insert(content()).observe(clicked);
        commands
            .entity(gui.child_by_name(root, "btnSend1").unwrap())
            .insert(Swap(list))
            .observe(swapped);
    }
}
fn verify(gui: &GuiQuery, list: Entity, rows: &Query<&GuiListRow>, reversed: bool) {
    let content = gui.list(list).unwrap();
    assert_eq!(content.item_count(), 80);
    assert!(!content.items().is_empty() && content.items().len() < 10);
    for &row in content.items() {
        let index = rows.get(row).unwrap().index();
        assert_eq!(
            gui.child_by_name(row, "name").is_some(),
            index.is_multiple_of(2) != reversed
        );
        assert_eq!(
            gui.text(gui.child_by_name(row, "msg").unwrap()),
            Some(message(index).as_str())
        );
        let msg = gui.child_by_name(row, "msg").unwrap();
        let size = gui.transform(msg).unwrap().size();
        assert!(size.x > 10. && size.x <= 663.);
        if gui.child_by_name(row, "name").is_none() {
            assert!((gui.transform(msg).unwrap().position().x + size.x - 785.).abs() < 0.1);
        }
        if index == 1 {
            assert!(size.y > 80. && gui.transform(row).unwrap().size().y > 100.);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    events: Res<Events>,
    demo: Res<Demo>,
    gui: GuiQuery,
    rows: Query<&GuiListRow>,
    status: Query<&GuiVirtualListState>,
    layouts: Query<(
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
        &ComputedNode,
    )>,
    existing: Query<()>,
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
        "provider smoke timeout step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
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
            state.owner = Some(
                commands
                    .spawn((
                        FairyGuiRoot::new(assets.load("ui/Chat.fui"), "Main"),
                        chat_demo::ChatFixture,
                    ))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(list) = gui.child_by_name(state.owner.unwrap(), "list") else {
                return;
            };
            verify(&gui, list, &rows, false);
            assert!(status.get(list).unwrap().error().is_none());
            state.list = Some(list);
            state.rows = gui.list(list).unwrap().items().to_vec();
            for &row in &state.rows {
                let text = gui.child_by_name(row, "msg").unwrap();
                let (block, info, node) = layouts.get(text).expect("rendered row text");
                let size = gui.transform(text).unwrap().size();
                assert!(info.scale_factor > 0.);
                assert!(
                    info.size.y / info.scale_factor <= size.y - 4. + 1.,
                    "rendered paragraph {:?} exceeds measured {:?} at scale {}",
                    info.size,
                    size,
                    info.scale_factor
                );
                assert!(
                    (node.size() * node.inverse_scale_factor() - size)
                        .abs()
                        .max_element()
                        < 1.
                );
                if rows.get(row).unwrap().index() == 0 {
                    assert_eq!(
                        block.buffer().lines().count(),
                        1,
                        "short text must fit without a DPI-induced extra line"
                    );
                }
            }
            let row = state.rows[1];
            state.clicked = Some(rows.get(row).unwrap().index());
            state.cursor = gui.local_to_global(row, Vec2::new(450., 50.));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-item-provider.png"));
            next = true;
        }
        2 | 6 | 12 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 7 | 13 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 6 => {
            assert_eq!(
                events.clicks,
                [(state.list.unwrap(), state.clicked.unwrap())]
            );
            let button = gui.child_by_name(state.owner.unwrap(), "btnSend1").unwrap();
            state.cursor = gui.local_to_global(button, gui.transform(button).unwrap().size() / 2.);
            next = true;
        }
        5 if elapsed >= 4 => {
            next = true;
        }
        8 if elapsed >= 8 => {
            assert_eq!(events.swaps, 1);
            let list = state.list.unwrap();
            verify(&gui, list, &rows, true);
            state
                .rows
                .extend_from_slice(gui.list(list).unwrap().items());
            commands.write_message(GuiObjectCommand::new(
                state.list.unwrap(),
                GuiObjectAction::ScrollTo {
                    position: Vec2::new(0., 6000.),
                    animated: false,
                },
            ));
            next = true;
        }
        9 if elapsed >= 8 => {
            let list = state.list.unwrap();
            verify(&gui, list, &rows, true);
            let current = gui.list(list).unwrap().items();
            assert!(
                current
                    .iter()
                    .all(|&row| rows.get(row).unwrap().index() >= 55)
            );
            assert!(current.iter().any(|row| state.rows.contains(row)));
            state.rows.extend_from_slice(current);
            commands.write_message(GuiObjectCommand::new(
                list,
                GuiObjectAction::ScrollTo {
                    position: Vec2::ZERO,
                    animated: false,
                },
            ));
            next = true;
        }
        10 if elapsed >= 8 => {
            let list = state.list.unwrap();
            verify(&gui, list, &rows, true);
            let row = gui.list(list).unwrap().items()[0];
            state.clicked = Some(rows.get(row).unwrap().index());
            state.cursor = gui.local_to_global(row, Vec2::new(450., 50.));
            next = true;
        }
        11 if elapsed >= 4 => {
            next = true;
        }
        14 if elapsed >= 8 => {
            assert_eq!(events.clicks.len(), 2);
            assert_eq!(
                events.clicks[1],
                (state.list.unwrap(), state.clicked.unwrap())
            );
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        15 if elapsed >= 8 => {
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: ECS mixed-template virtual rows, synchronous wrapped/inline-image text sizing and right-edge/row-height relations, row click observer, button-driven provider replacement, long scroll/reuse, rebound row clicks and pool disposal"
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
