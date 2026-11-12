use super::*;
use bevy::input::keyboard::Key;
use fairygui_core::widget::ListSelectionMode;

fn fixture() -> (App, GObjectId, Vec<GObjectId>, Vec<Entity>) {
    let (mut app, _) = setup();
    let package = fairygui_core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let (list, rows) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui
            .create_root(Handle::default(), &package, "Demo_List")
            .unwrap();
        let root = gui.root_object(root).unwrap();
        let list = gui.store_mut().create(ObjectType::List);
        gui.store_mut().set_parent(list, Some(root));
        let rows = (0..8)
            .map(|_| {
                let row = fairygui_core::widget::create_from_package(
                    gui.store_mut(),
                    &package,
                    package.item_index_by_name("Checkbox").unwrap(),
                    &fairygui_core::package::LoadContext::none(),
                )
                .unwrap();
                gui.store_mut().set_parent(row, Some(list));
                row
            })
            .collect::<Vec<_>>();
        gui.store_mut().attach_to_stage(root);
        gui.store_mut()
            .set_list_selection_mode(list, ListSelectionMode::Multiple);
        (list, rows)
    };
    let mut entities = Vec::new();
    for &row in &rows {
        let (entity, _) = target(&mut app, 20);
        app.world_mut()
            .entity_mut(entity)
            .insert((GObjectRef(row), Hovered(false)));
        entities.push(entity);
    }
    (app, list, rows, entities)
}

fn click(app: &mut App, entities: &[Entity], index: usize, shift: bool, ctrl: bool) {
    for (i, &entity) in entities.iter().enumerate() {
        app.world_mut()
            .entity_mut(entity)
            .insert(Hovered(i == index));
    }
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<Key>>();
        keys.reset_all();
        if shift {
            keys.press(Key::Shift);
        }
        if ctrl {
            keys.press(Key::Control);
        }
    }
    press(app);
    release(app);
}

#[test]
fn registered_bar_track_and_arrows_scroll_without_row_events_or_selection() {
    use fairygui_core::scroll::{ARROW1_NAME, ARROW2_NAME, BAR_NAME};
    let (mut app, list, rows, mut entities) = fixture();
    let package = fairygui_core::FuiPackage::parse(
        "Chat",
        include_bytes!("../../../fairygui-core/tests/fixtures/Chat.fui"),
    )
    .unwrap();
    let parts = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = gui.root_object(root).unwrap();
        let mut descendants = Vec::new();
        gui.store()
            .for_each_descendant(root, &mut |id| descendants.push(id));
        let source = descendants
            .into_iter()
            .find(|&id| {
                matches!(
                    gui.store().object(id).unwrap().control(),
                    fairygui_core::widget::ControlData::List(_)
                ) && gui.store().scroll_pane(id).is_some()
            })
            .unwrap();
        // Move the selectable rows into an actual exported Chat scroll pane.
        gui.store_mut()
            .attach_scroll_bars(source, &mut |store, url| {
                let index = package.resolve_local_url(url)?;
                fairygui_core::widget::create_from_package(
                    store,
                    &package,
                    index,
                    &fairygui_core::package::LoadContext::none(),
                )
                .ok()
            });
        let bar = gui.store().scroll_pane(source).unwrap().vt_bar.unwrap();
        for &row in &rows {
            gui.store_mut().set_parent(row, Some(source));
            gui.store_mut().set_size(row, 180., 40.);
        }
        gui.store_mut().set_size(source, 200., 100.);
        gui.store_mut()
            .set_list_selection_mode(source, ListSelectionMode::Multiple);
        gui.store_mut().layout_list(source);
        gui.store_mut().layout_scroll_bars(source);
        gui.store_mut().add_child_at(source, bar, 0);
        // A containing button must not steal scrollbar background capture.
        let wrapper = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_parent(wrapper, Some(root));
        gui.store_mut().set_parent(source, Some(wrapper));
        gui.store_mut().attach_to_stage(root);
        let track = gui.store().child_by_name(bar, BAR_NAME).unwrap();
        // Include synthetic named arrows when the exported skin has none.
        let mut parts = vec![track];
        for name in [ARROW1_NAME, ARROW2_NAME] {
            let part = gui.store().child_by_name(bar, name).unwrap_or_else(|| {
                let part = gui.store_mut().create(ObjectType::Component);
                gui.store_mut().set_name(part, name);
                gui.store_mut().set_parent(part, Some(bar));
                part
            });
            parts.push(part);
        }
        parts.push(bar);
        let label = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(label, Some(parts[1]));
        gui.store_mut().set_size(label, 10., 10.);
        parts.push(label);
        let decor = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(decor, Some(track));
        let size = gui.store().object(track).unwrap().size();
        gui.store_mut().set_size(decor, size.width, size.height);
        parts.push(decor);
        parts.push(
            gui.store()
                .child_by_name(bar, fairygui_core::scroll::GRIP_NAME)
                .unwrap(),
        );
        assert_eq!(gui.store().list_item_count(source), rows.len());
        (source, parts)
    };
    let (source, parts) = parts;
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_item_count(list),
        0
    );
    app.add_message::<GuiListItemClick>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_list_item_messages.after(GuiSystems::Input),
    );
    for &part in &parts {
        let (entity, _) = target(&mut app, 99);
        app.world_mut()
            .entity_mut(entity)
            .insert((GObjectRef(part), Hovered(false)));
        entities.push(entity);
    }
    click(&mut app, &entities, 2, false, false);
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiListItemClick>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        (
            messages[0].event.target,
            messages[0].item,
            messages[0].index
        ),
        (source, rows[2], 2)
    );
    for (offset, direction) in [
        (0, 1.),
        (1, -1.),
        (2, 1.),
        (3, 1.),
        (4, -1.),
        (5, 1.),
        (6, 0.),
    ] {
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_scroll_pos(source, 0., 60., false);
            gui.store_mut().sync_scroll_bars(source);
            gui.drain_events().for_each(drop);
            let mut ancestor = Some(parts[offset]);
            while let Some(id) = ancestor {
                let object = gui.store().object(id).unwrap();
                assert!(
                    object.touchable() && gui.store().display_visible(id),
                    "part={offset} ancestor={} touchable={} visible={}",
                    object.name(),
                    object.touchable(),
                    gui.store().display_visible(id)
                );
                ancestor = object.parent();
            }
        }
        for (i, &entity) in entities.iter().enumerate() {
            app.world_mut()
                .entity_mut(entity)
                .insert(Hovered(i == rows.len() + offset));
        }
        {
            let mut windows = app.world_mut().query::<&mut Window>();
            for mut window in windows.iter_mut(app.world_mut()) {
                window.set_cursor_position(Some(Vec2::new(10., 10.)));
            }
        }
        press(&mut app);
        assert_eq!(
            app.world().resource::<GuiInputState>().captured(),
            Some(parts[offset])
        );
        let after_down = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(source)
            .unwrap()
            .pos();
        if direction == 0. {
            assert_eq!(after_down.y, 60., "grip press does not page");
        } else {
            assert!(
                (after_down.y - 60.) * direction > 0.,
                "part={offset} must act on press"
            );
        }
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let events: Vec<_> = gui.drain_events().collect();
            assert!(
                !events
                    .iter()
                    .any(|event| event.kind == EventKind::ClickItem)
            );
            let downs: Vec<_> = events
                .iter()
                .filter(|event| event.kind == EventKind::MouseDown)
                .collect();
            assert!(!downs.is_empty());
            let stopped = matches!(offset, 1 | 2 | 4 | 6);
            let wrapper = gui.store().object(source).unwrap().parent().unwrap();
            let fairygui_core::widget::ControlData::Button(wrapper) =
                gui.store().object(wrapper).unwrap().control()
            else {
                panic!("button wrapper");
            };
            assert_eq!(
                wrapper.pressed, !stopped,
                "stopped MouseDown cannot press an outer button"
            );
            assert_eq!(
                downs.iter().any(|event| event.current_target == source),
                !stopped
            );
            if offset == 4 {
                assert_eq!(
                    downs.last().unwrap().current_target,
                    parts[1],
                    "nested hit stops at arrow"
                );
            }
        }
        // Holding and moving never repeat a track/arrow press or drag content.
        if offset % 2 == 0 && offset != 6 {
            for &entity in &entities {
                app.world_mut().entity_mut(entity).insert(Hovered(false));
            }
            let mut windows = app.world_mut().query::<&mut Window>();
            for mut window in windows.iter_mut(app.world_mut()) {
                window.set_cursor_position(Some(Vec2::new(100., 180.)));
            }
        }
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .scroll_pane(source)
                .unwrap()
                .pos(),
            after_down
        );
        release(&mut app);
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let pane = gui.store().scroll_pane(source).unwrap();
        assert_eq!(
            pane.pos(),
            after_down,
            "release adds no action, inside or outside"
        );
        if direction != 0. {
            assert!(
                (pane.pos().y - 60.) * direction > 0.,
                "part={offset} offset={}",
                pane.pos().y
            );
        }
        assert!(!pane.is_animating(), "bar clicks do not request row reveal");
        assert_eq!(gui.store().list_selections(source), Some(vec![2]));
        assert!(
            !gui.drain_events()
                .any(|event| event.kind == EventKind::ClickItem)
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<GuiListItemClick>>()
                .drain()
                .count(),
            0
        );
    }
}

#[test]
fn actual_pointer_router_applies_modifiers_and_emits_one_targeted_item_message_after_selection() {
    let (mut app, list, rows, entities) = fixture();
    app.add_message::<GuiListItemClick>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_list_item_messages.after(GuiSystems::Input),
    );
    for (index, shift, ctrl, expected) in [
        (1, false, false, vec![1]),
        (4, false, true, vec![1, 4]),
        (2, true, false, vec![1, 2, 3, 4]),
        (3, false, false, vec![3]),
        (3, false, true, vec![]),
    ] {
        click(&mut app, &entities, index, shift, ctrl);
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert_eq!(gui.store().list_selections(list), Some(expected.clone()));
        for (i, &row) in rows.iter().enumerate() {
            assert_eq!(
                gui.store().button_selected(row),
                Some(expected.contains(&i))
            );
        }
        let events: Vec<_> = gui.drain_events().collect();
        assert_eq!(
            events
                .iter()
                .filter(|e| e.kind == EventKind::StateChanged)
                .count(),
            0,
            "list row buttons do not perform their own click toggle"
        );
        let items: Vec<_> = events
            .iter()
            .filter(|e| e.kind == EventKind::ClickItem)
            .collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].target, list);
        assert_eq!(items[0].current_target, list);
        assert!(!items[0].bubbling);
        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<GuiListItemClick>>()
            .drain()
            .collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].item, rows[index]);
        assert_eq!(messages[0].index, index);
        assert_eq!((messages[0].shift, messages[0].ctrl), (shift, ctrl));
    }
}

#[test]
fn single_click_toggle_none_mode_nested_hit_and_drag_suppression_keep_native_selection_rules() {
    let (mut app, list, rows, mut entities) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, ListSelectionMode::MultipleSingleClick);
    click(&mut app, &entities, 0, false, false);
    click(&mut app, &entities, 3, false, false);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![0, 3])
    );
    click(&mut app, &entities, 0, false, false);
    let nested = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let child = gui.store_mut().create(ObjectType::Text);
        gui.store_mut().set_parent(child, Some(rows[2]));
        child
    };
    let (entity, _) = target(&mut app, 99);
    app.world_mut()
        .entity_mut(entity)
        .insert((GObjectRef(nested), Hovered(false)));
    entities.push(entity);
    click(&mut app, &entities, entities.len() - 1, false, false);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![2, 3])
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, ListSelectionMode::None);
    click(&mut app, &entities, 1, true, true);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![2, 3])
    );
    // A press/release over different row targets never completes a click.
    for &e in &entities {
        app.world_mut().entity_mut(e).insert(Hovered(false));
    }
    app.world_mut()
        .entity_mut(entities[0])
        .insert(Hovered(true));
    press(&mut app);
    app.world_mut()
        .entity_mut(entities[0])
        .insert(Hovered(false));
    app.world_mut()
        .entity_mut(entities[1])
        .insert(Hovered(true));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);
    release(&mut app);
    assert!(
        !app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .drain_events()
            .any(|e| e.kind == EventKind::ClickItem)
    );
}

#[test]
fn scroll_view_clicks_animate_exported_lists_in_none_mode_and_respect_the_runtime_flag() {
    use fairygui_core::widget::{ControlData, ListLayoutType};
    let (mut app, _) = setup();
    let package = fairygui_core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let (list, row) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui
            .create_root(Handle::default(), &package, "Demo_List")
            .unwrap();
        let root = gui.root_object(root).unwrap();
        let list = gui.store().iter().find_map(|(id, o)| {
            matches!(o.control(), ControlData::List(data) if data.layout == ListLayoutType::SingleColumn
                && data.component.scroll_pane.is_some()).then_some(id)
        }).unwrap();
        gui.store_mut().set_size(list, 200., 100.);
        let mut row = None;
        for _ in 0..8 {
            let item = fairygui_core::widget::create_from_package(
                gui.store_mut(),
                &package,
                package.item_index_by_name("Checkbox").unwrap(),
                &fairygui_core::package::LoadContext::none(),
            )
            .unwrap();
            gui.store_mut().set_parent(item, Some(list));
            gui.store_mut().set_size(item, 180., 40.);
            row = Some(item);
        }
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        gui.store_mut().attach_to_stage(root);
        gui.store_mut()
            .set_list_selection_mode(list, ListSelectionMode::None);
        gui.store_mut()
            .set_list_scroll_item_to_view_on_click(list, true);
        (list, row.unwrap())
    };
    let (entity, _) = target(&mut app, 30);
    app.world_mut().entity_mut(entity).insert(GObjectRef(row));
    press(&mut app);
    release(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert_eq!(gui.store().list_selections(list), Some(vec![]));
        let pane = gui.store().scroll_pane(list).unwrap();
        assert!(pane.pos().y > 0. && pane.is_animating());
        assert_eq!(pane.display_pos().y, 0.);
        assert_eq!(
            gui.drain_events()
                .filter(|e| e.kind == EventKind::ClickItem)
                .count(),
            1
        );
        gui.store_mut().advance_scrolls(0.5);
        gui.store_mut().set_scroll_pos(list, 0., 0., false);
        gui.store_mut()
            .set_list_scroll_item_to_view_on_click(list, false);
    }
    press(&mut app);
    release(&mut app);
    let gui = app.world().non_send::<GuiRuntime>();
    let pane = gui.store().scroll_pane(list).unwrap();
    assert_eq!(pane.pos().y, 0.);
    assert!(!pane.is_animating());
}
