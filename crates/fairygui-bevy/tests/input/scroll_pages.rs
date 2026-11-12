use super::*;
use fairygui_core::{
    FuiPackage,
    geom::Size,
    package::LoadContext,
    widget::{ControlData, create_from_package},
};

#[test]
fn exported_bag_page_button_and_wheel_drive_each_other_without_selecting_list_items() {
    let (mut app, window) = setup();
    let package = FuiPackage::parse(
        "Bag",
        include_bytes!("../../../fairygui-core/tests/fixtures/Bag.fui"),
    )
    .unwrap();
    let (owner, list, pager, page_button, name, page) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "BagWin")
            .unwrap();
        let owner = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(owner);
        let list = gui.store().child_by_name(owner, "list").unwrap();
        gui.store_mut().layout_scroll_bars(list);
        let view = gui.store().scroll_pane(list).unwrap().view_size();
        gui.store_mut()
            .update_scroll_content_size(list, Size::new(view.width * 3., view.height));
        gui.store_mut().initialize_scroll_page_controllers(owner);
        let controller = &gui
            .store()
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controllers[0];
        let name = controller.name.clone();
        let pager = gui.store().child_by_name(owner, "n25").unwrap();
        let ControlData::List(data) = gui.store().object(pager).unwrap().control() else {
            panic!("pager")
        };
        let (package, item) = gui
            .resolve_url(data.default_item.as_ref().unwrap())
            .map(|(p, i)| (p.clone(), i))
            .unwrap();
        for _ in 0..3 {
            let row =
                create_from_package(gui.store_mut(), &package, item, &LoadContext::none()).unwrap();
            gui.store_mut().set_parent(row, Some(pager));
        }
        gui.store_mut().initialize_list_selections(owner);
        let button = gui.store().list_item_at(pager, 2).unwrap();
        (owner, list, pager, button, name, view.width)
    };
    let (entity, _) = target(&mut app, 100);
    app.world_mut()
        .entity_mut(entity)
        .insert(GObjectRef(page_button));
    press(&mut app);
    release(&mut app);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_current_page_x(list),
        Some(0),
        "the exported page indicator is untouchable"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_touchable(pager, true);
    press(&mut app);
    release(&mut app);
    {
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.store().scroll_current_page_x(list), Some(2));
        assert_eq!(gui.store().scroll_pane(list).unwrap().pos().x, page * 2.);
        assert_eq!(gui.store().button_selected(page_button), Some(true));
    }
    app.world_mut().entity_mut(entity).insert(GObjectRef(list));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_mouse_wheel_enabled(list, true);
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.,
        y: 1.,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.store().scroll_current_page_x(list), Some(1));
    assert_eq!(
        gui.store()
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller(&name)
            .unwrap()
            .selected_index,
        1
    );
    assert_eq!(gui.store().button_selected(page_button), Some(false));
    assert!(gui.store().list_selections(list).unwrap().is_empty());
}
