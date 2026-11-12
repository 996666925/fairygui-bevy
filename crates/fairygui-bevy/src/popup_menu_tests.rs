use super::*;
use fairygui_core::popup::MenuItem;

fn templates(separator: bool) -> PopupMenuTemplates<'static> {
    PopupMenuTemplates {
        content: "PopupMenu",
        item: "PopupMenuItem",
        // A real non-button package image exercises the separate resource pool.
        // Basics has no standalone horizontal separator; appearance is caller-owned.
        separator: separator.then_some("VtSeperator"),
    }
}

fn show(app: &mut App, package: &FuiPackage, items: &[MenuItem]) -> GObjectId {
    with_commands(app, |gui, commands| {
        gui.show_popup_menu_items(
            commands,
            package,
            templates(true),
            items,
            Point::new(20.0, 30.0),
        )
        .unwrap()
    })
}

fn checked_index(gui: &GuiRuntime, row: GObjectId) -> i32 {
    gui.store
        .object(row)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller("checked")
        .unwrap()
        .selected_index
}

#[test]
fn named_action_waits_toggles_closes_then_publishes_payload_and_can_reopen_in_respond() {
    #[derive(Resource, Default)]
    struct Received(Vec<GuiMenuAction>);
    fn receive(
        mut messages: MessageReader<GuiMenuAction>,
        mut gui: NonSendMut<GuiRuntime>,
        mut received: ResMut<Received>,
    ) {
        for message in messages.read() {
            assert!(
                !gui.is_popup_menu_open(),
                "menu closed before application handler"
            );
            assert!(!gui.popup_stack().contains(&message.menu));
            assert_eq!(checked_index(&gui, message.item), 2);
            gui.update_popup_menu_item(&message.name, |item| item.caption = "Handled".into());
            gui.reopen_popup_menu(Point::ZERO).unwrap();
            received.0.push(message.clone());
        }
    }
    let (mut app, package, _) = fixture();
    let item = MenuItem {
        checkable: true,
        ..MenuItem::action("toggle", "Toggle", 73)
    };
    let content = show(&mut app, &package, &[item]);
    let row = app
        .world()
        .non_send::<GuiRuntime>()
        .popup_menu_item("toggle")
        .unwrap()
        .1;
    app.add_message::<GuiMenuAction>()
        .init_resource::<Received>()
        .configure_sets(
            Update,
            (
                crate::GuiSystems::Publish,
                crate::GuiSystems::Respond,
                crate::GuiSystems::Sync,
            )
                .chain(),
        )
        .add_systems(Update, emit_menu_actions.in_set(crate::GuiSystems::Publish))
        .add_systems(Update, receive.in_set(crate::GuiSystems::Respond));
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.activate_menu_item(row, Some(Point::new(42.0, 84.0)));
        let list = gui.store.child_by_name(content, "list").unwrap();
        assert_eq!(gui.store.list_selection(list), Some(0));
        gui.advance_menu_click(0.099);
        assert!(gui.is_popup_menu_open());
        assert!(gui.menu_actions.is_empty());
        gui.advance_menu_click(0.002);
        assert!(!gui.is_popup_menu_open());
    }
    app.update();
    let received = &app.world().resource::<Received>().0;
    assert_eq!(received.len(), 1);
    let message = &received[0];
    assert_eq!((message.menu, message.item), (content, row));
    assert_eq!(
        (&*message.name, &*message.caption, message.action),
        ("toggle", "Toggle", Some(73))
    );
    assert_eq!(message.pointer, Some(Point::new(42.0, 84.0)));
    assert!(message.checked);
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.popup_menu_content(), Some(content));
    assert_eq!(gui.popup_menu_item("toggle").unwrap().0.caption, "Handled");
    assert_eq!(
        gui.store.button_selected(row),
        Some(false),
        "show clears highlight"
    );
    app.update();
    assert_eq!(
        app.world().resource::<Received>().0.len(),
        1,
        "exactly one activation"
    );
}

#[test]
fn edits_preserve_row_identity_across_reopen_and_use_the_template_checked_controller() {
    let (mut app, package, _) = fixture();
    let config = [MenuItem::action("save", "Save", 12)];
    let content = show(&mut app, &package, &config);
    app.update();
    let (row, entity) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let row = gui.popup_menu_item("save").unwrap().1;
        (row, gui.entity_of(row))
    };
    let icon = format!(
        "ui://{}{}",
        package.id(),
        package
            .item(package.item_index_by_name("VtSeperator").unwrap())
            .unwrap()
            .id
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert!(gui.update_popup_menu_item("save", |item| {
            item.caption = "Save as".into();
            item.icon = Some(icon.clone());
            item.checked = true;
            item.enabled = false;
            item.action = Some(99);
        }));
        assert_eq!(checked_index(&gui, row), 2);
        assert!(gui.store.object(row).unwrap().grayed());
        assert!(!gui.update_popup_menu_item("save", |item| item.name = "renamed".into()));
        assert!(!gui.update_popup_menu_item("save", |item| item.separator = true));
        gui.hide_popup(Some(content));
    }
    assert_eq!(show(&mut app, &package, &config), content);
    app.update();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    let (item, object) = gui.popup_menu_item("save").unwrap();
    assert_eq!(object, row);
    assert_eq!(gui.entity_of(row), entity);
    assert_eq!(
        (&*item.caption, item.action, &item.icon),
        ("Save as", Some(99), &Some(icon.clone()))
    );
    let title = gui.store.child_by_name(row, "title").unwrap();
    assert_eq!(
        gui.store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Save as"
    );
    let loader = gui.store.child_by_name(row, "icon").unwrap();
    assert!(
        matches!(gui.store.object(loader).unwrap().control(), ControlData::Loader(data) if data.url.as_deref() == Some(&icon))
    );
    gui.update_popup_menu_item("save", |item| item.enabled = true);
    gui.activate_menu_item(row, None);
    gui.advance_menu_click(0.1);
    assert!(!gui.menu_actions[0].checked);
    assert_eq!(checked_index(&gui, row), 1);
    gui.update_popup_menu_item("save", |item| {
        item.checked = false;
        item.checkable = false;
    });
    assert_eq!(checked_index(&gui, row), 0);
}

#[test]
fn hidden_disabled_removed_detached_and_recycled_rows_never_activate_old_actions() {
    let (mut app, package, _) = fixture();
    let config = [MenuItem::action("old", "Old", 1)];
    for invalidation in 0..7 {
        show(&mut app, &package, &config);
        let row = {
            let gui = app.world().non_send::<GuiRuntime>();
            gui.popup_menu_item("old").unwrap().1
        };
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.activate_menu_item(row, None);
            match invalidation {
                0 => {
                    gui.update_popup_menu_item("old", |item| item.enabled = false);
                }
                1 => {
                    gui.update_popup_menu_item("old", |item| item.visible = false);
                }
                2 => {
                    gui.store.set_grayed(row, true);
                }
                3 => {
                    gui.store.detach_from_parent(row);
                }
                4 => {
                    assert!(gui.remove_popup_menu_item("old"));
                    let replacement = gui
                        .add_popup_menu_item(&package, MenuItem::action("new", "New", 2))
                        .unwrap();
                    assert_eq!(replacement, row, "exercise the reused identity");
                }
                5 => {
                    gui.hide_popup(None);
                    gui.reopen_popup_menu(Point::ZERO).unwrap();
                }
                6 => {
                    gui.clear_popup_menu_items();
                }
                _ => unreachable!(),
            }
            gui.advance_menu_click(1.0);
            assert!(gui.menu_actions.is_empty(), "invalidation {invalidation}");
            assert!(gui.is_popup_menu_open());
            // Clear dynamic state so the next iteration gets a fresh definition.
            gui.clear_popup_menu_items();
            gui.add_popup_menu_item(&package, config[0].clone())
                .unwrap();
        }
    }
}

#[test]
fn replacement_cancels_pending_action_and_invalid_configuration_preserves_open_menu() {
    let (mut app, package, _) = fixture();
    let content = show(&mut app, &package, &[MenuItem::action("a", "A", 1)]);
    let row = app
        .world()
        .non_send::<GuiRuntime>()
        .popup_menu_item("a")
        .unwrap()
        .1;
    with_commands(&mut app, |gui, commands| {
        gui.activate_menu_item(row, None);
        let duplicate = [MenuItem::new("same"), MenuItem::new("same")];
        assert!(
            gui.show_popup_menu_items(commands, &package, templates(true), &duplicate, Point::ZERO)
                .is_none()
        );
        assert!(
            gui.show_popup_menu_items(
                commands,
                &package,
                templates(false),
                &[MenuItem::separator()],
                Point::ZERO
            )
            .is_none()
        );
        let invalid = PopupMenuTemplates {
            item: "VtSeperator",
            ..templates(false)
        };
        assert!(
            gui.show_popup_menu_items(
                commands,
                &package,
                invalid,
                &[MenuItem::new("bad")],
                Point::ZERO
            )
            .is_none()
        );
        assert_eq!(gui.popup_menu_content(), Some(content));
        assert_eq!(gui.popup_menu_item("a").unwrap().1, row);
        assert!(gui.store.contains(row));
        gui.show_popup_menu_items(
            commands,
            &package,
            templates(true),
            &[MenuItem::action("b", "B", 2)],
            Point::ZERO,
        )
        .unwrap();
        gui.advance_menu_click(1.0);
        assert!(gui.menu_actions.is_empty());
        assert!(gui.is_popup_menu_open());
        assert!(gui.popup_menu_item("a").is_none());
    });
}

#[test]
fn separators_use_their_own_pool_and_hidden_rows_follow_fold_setting_without_changing_indices() {
    let (mut app, package, _) = fixture();
    let separator = MenuItem {
        name: "separator".into(),
        ..MenuItem::separator()
    };
    let config = [
        MenuItem::new("first"),
        separator.clone(),
        MenuItem::new("last"),
    ];
    let content = show(&mut app, &package, &config);
    app.update();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    let list = gui.store.child_by_name(content, "list").unwrap();
    let first = gui.popup_menu_item("first").unwrap().1;
    let line = gui.popup_menu_item("separator").unwrap().1;
    let last = gui.popup_menu_item("last").unwrap().1;
    assert!(!matches!(
        gui.store.object(line).unwrap().control(),
        ControlData::Button(_)
    ));
    gui.activate_menu_item(line, None);
    gui.advance_menu_click(1.0);
    assert!(gui.is_popup_menu_open());
    assert!(gui.menu_actions.is_empty());
    assert!(gui.remove_popup_menu_item("separator"));
    assert_eq!(gui.add_popup_menu_item(&package, separator), Some(line));
    assert_eq!(
        gui.popup_menu_hit(last),
        Some(Some(1)),
        "logical row index after removal"
    );
    let full = gui.store.object(list).unwrap().height();
    let last_y = gui.store.object(last).unwrap().y();
    gui.store.set_list_fold_invisible_items(list, false);
    gui.update_popup_menu_item("first", |item| item.visible = false);
    assert_eq!(gui.store.object(last).unwrap().y(), last_y);
    gui.store.set_list_fold_invisible_items(list, true);
    assert_eq!(
        gui.store.object(last).unwrap().y(),
        gui.store.object(first).unwrap().y()
    );
    assert_eq!(
        gui.store.object(list).unwrap().height(),
        full,
        "visibility edit reflows but frame fits on show"
    );
    gui.reopen_popup_menu(Point::ZERO).unwrap();
    assert!(gui.store.object(list).unwrap().height() < full);
    assert_eq!(
        gui.popup_menu_hit(last),
        Some(Some(1)),
        "visibility never shifts action indices"
    );
    gui.activate_menu_item(last, None);
    gui.advance_menu_click(0.1);
    assert_eq!(gui.menu_actions.len(), 1);
    assert_eq!(gui.menu_actions[0].name, "last");
    gui.clear_popup_menu_items();
    gui.reopen_popup_menu(Point::ZERO).unwrap();
    assert_eq!(gui.store.object(list).unwrap().height(), 10.0);
}

#[test]
fn disposing_a_retained_menu_reclaims_views_and_external_content_removal_clears_open_state() {
    let (mut app, package, _) = fixture();
    let config = [MenuItem::action("a", "A", 1)];
    let content = show(&mut app, &package, &config);
    app.update();
    let (entity, row, row_entity) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let row = gui.popup_menu_item("a").unwrap().1;
        (
            gui.entity_of(content).unwrap(),
            row,
            gui.entity_of(row).unwrap(),
        )
    };
    with_commands(&mut app, |gui, commands| {
        gui.activate_menu_item(row, None);
        assert!(gui.dispose_popup_menu(commands));
        assert!(!gui.dispose_popup_menu(commands));
        gui.advance_menu_click(1.0);
        assert!(gui.menu_actions.is_empty());
    });
    app.update();
    assert!(app.world().get_entity(entity).is_err());
    assert!(app.world().get_entity(row_entity).is_err());
    let new_content = show(&mut app, &package, &config);
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .remove(new_content);
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.is_popup_menu_open());
    assert!(gui.popup_stack().is_empty());
    assert!(gui.retained_menu.is_none());
}

#[test]
fn overlay_root_keeps_its_global_layer_when_later_pages_are_created_and_respects_application_override()
 {
    let (mut app, package, _) = fixture();
    let content = show(&mut app, &package, &[MenuItem::new("first")]);
    app.update();
    let root_entity = {
        let gui = app.world().non_send::<GuiRuntime>();
        let root = gui.root_container().unwrap();
        let entity = gui.entity_of(root).unwrap();
        assert_eq!(app.world().get::<GlobalZIndex>(entity).unwrap().0, 1);
        assert_eq!(
            app.world()
                .get::<ChildOf>(gui.entity_of(content).unwrap())
                .unwrap()
                .parent(),
            entity
        );
        entity
    };
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let handle = gui.asset_handle_for(content).unwrap();
        gui.create_root(handle, &package, "Demo_Popup").unwrap();
    }
    app.update();
    assert_eq!(app.world().get::<GlobalZIndex>(root_entity).unwrap().0, 1);
    app.world_mut()
        .entity_mut(root_entity)
        .insert(GlobalZIndex(500));
    app.update();
    assert_eq!(app.world().get::<GlobalZIndex>(root_entity).unwrap().0, 500);
}
