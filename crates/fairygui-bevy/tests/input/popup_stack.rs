use super::*;

#[test]
fn menu_row_release_keeps_immediate_click_and_publishes_delayed_action_with_original_pointer() {
    use bevy::ecs::system::RunSystemOnce;
    use fairygui_core::{geom::Point, popup::MenuItem};
    use std::time::Duration;

    let (mut app, combo_entity, combo) = fixture();
    app.world_mut()
        .entity_mut(combo_entity)
        .insert(Hovered(false));
    app.add_message::<GuiMenuAction>()
        .configure_sets(Update, (GuiSystems::Input, GuiSystems::Publish).chain())
        .add_systems(
            Update,
            fairygui_bevy::runtime::emit_menu_actions.in_set(GuiSystems::Publish),
        );
    let package = fairygui_core::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let row = app
        .world_mut()
        .run_system_once(
            move |mut commands: Commands, mut gui: NonSendMut<GuiRuntime>| {
                gui.ensure_root(fairygui_core::geom::Size::new(1280.0, 720.0));
                gui.show_popup_menu_items(
                    &mut commands,
                    &package,
                    PopupMenuTemplates {
                        content: "PopupMenu",
                        item: "PopupMenuItem",
                        separator: None,
                    },
                    &[MenuItem {
                        checkable: true,
                        ..MenuItem::action("toggle", "Toggle", 912)
                    }],
                    Point::ZERO,
                )
                .unwrap();
                gui.popup_menu_item("toggle").unwrap().1
            },
        )
        .unwrap();
    let (row_entity, _) = target(&mut app, 40);
    app.world_mut()
        .entity_mut(row_entity)
        .insert(GObjectRef(row));
    press(&mut app);
    release(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert!(gui.is_popup_menu_open());
        assert!(gui.drain_events().any(|event| {
            event.kind == EventKind::Click && event.pointer == Some(Point::new(10.0, 10.0))
        }));
    }
    assert!(
        app.world_mut()
            .resource_mut::<Messages<GuiMenuAction>>()
            .drain()
            .next()
            .is_none()
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(99));
    app.update();
    assert!(app.world().non_send::<GuiRuntime>().is_popup_menu_open());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(2));
    app.update();
    let actions: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiMenuAction>>()
        .drain()
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(
        (&*actions[0].name, actions[0].action, actions[0].item),
        ("toggle", Some(912), row)
    );
    assert!(actions[0].checked);
    assert_eq!(actions[0].pointer, Some(Point::new(10.0, 10.0)));
    assert!(!app.world().non_send::<GuiRuntime>().is_popup_menu_open());
    assert!(app.world().non_send::<GuiRuntime>().store().contains(combo));
}

fn fixture() -> (App, Entity, GObjectId) {
    let (mut app, _) = setup();
    let package = fairygui_core::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let combo = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui.create_root(handle, &package, "Demo_ComboBox").unwrap();
        let root = gui.root_object(root).unwrap();
        gui.store_mut().attach_to_stage(root);
        gui.store().child_by_name(root, "n1").unwrap()
    };
    let (entity, _) = target(&mut app, 10);
    app.world_mut().entity_mut(entity).insert(GObjectRef(combo));
    (app, entity, combo)
}

#[test]
fn combo_opens_on_press_stays_open_on_release_and_reuses_the_same_dropdown_after_toggling() {
    let (mut app, _, combo) = fixture();
    press(&mut app);
    let first = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .combo_popup(combo)
        .unwrap();
    assert_eq!(
        app.world().non_send::<GuiRuntime>().popup_stack(),
        &[first.dropdown]
    );
    release(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .combo_popup(combo)
            .is_some()
    );
    press(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .combo_popup(combo)
            .is_none()
    );
    release(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .popup_stack()
            .is_empty(),
        "release cannot reopen a just-closed dropdown"
    );
    press(&mut app);
    let second = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .combo_popup(combo)
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn combo_item_click_closes_the_complete_mixed_stack_and_emits_state_changed_even_when_reselected() {
    let (mut app, combo_entity, combo) = fixture();
    for _ in 0..2 {
        app.world_mut()
            .entity_mut(combo_entity)
            .insert(Hovered(true));
        press(&mut app);
        release(&mut app);
        let (item, parent) = {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let parent = gui.store_mut().create(ObjectType::Component);
            // Put a second generic popup above the dropdown. Pressing a row
            // closes only this top entry; releasing the row closes the rest.
            gui.show_popup(parent, fairygui_core::geom::Point::ZERO);
            let popup = gui.store().combo_popup(combo).unwrap();
            (gui.store().child_at(popup.list, 2).unwrap(), parent)
        };
        let (item_entity, _) = target(&mut app, 30);
        app.world_mut()
            .entity_mut(item_entity)
            .insert(GObjectRef(item));
        app.world_mut()
            .entity_mut(combo_entity)
            .insert(Hovered(false));
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .drain_events()
            .for_each(drop);
        press(&mut app);
        assert!(
            !app.world()
                .non_send::<GuiRuntime>()
                .popup_stack()
                .contains(&parent)
        );
        release(&mut app);
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            assert!(gui.popup_stack().is_empty());
            let events: Vec<_> = gui
                .drain_events()
                .filter(|event| {
                    event.kind == EventKind::StateChanged
                        && event.target == combo
                        && event.is_at_target()
                })
                .collect();
            assert_eq!(events.len(), 1);
            assert!(events[0].pointer.is_some());
            assert!(matches!(gui.store().object(combo).unwrap().control(),
            fairygui_core::widget::ControlData::ComboBox(data) if data.selected_index == 2 && data.title == "Item 3"));
        }
        app.world_mut()
            .entity_mut(item_entity)
            .insert(Hovered(false));
    }
}
