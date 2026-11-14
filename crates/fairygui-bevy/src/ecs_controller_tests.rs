use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(item: &str, loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), item))
        .id();
    if loaded {
        load(&mut app, &asset, "Basics");
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>, name: &str) {
    let bytes: &[u8] = match name {
        "Basics" => include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
        "Bag" => include_bytes!("../../fairygui-demo/assets/ui/Bag.fui"),
        _ => unreachable!(),
    };
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(name, bytes).unwrap(),
                res_key: format!("ui/{name}"),
            },
        )
        .unwrap();
}
fn child(app: &mut App, owner: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}
fn index(app: &App, view: Entity, name: &str) -> i32 {
    app.world()
        .get::<GuiControllers>(view)
        .unwrap()
        .get(name)
        .unwrap()
        .selected_index
}

#[test]
fn delayed_initial_pages_apply_before_initialize_and_publish_exported_metadata() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            let controllers = gui.controllers(owner).unwrap();
            assert_eq!(controllers.states().len(), 2);
            assert!(controllers.get("missing").is_none());
            for name in ["c1", "c2"] {
                let state = controllers.get(name).unwrap();
                assert_eq!(state.selected_index, 1);
                assert_eq!(state.previous_index(), 0);
                assert_eq!(state.page_ids(), ["0", "1"]);
                assert_eq!(state.selected_page_id(), Some("1"));
            }
            let image = gui.child_by_name(owner, "n8").unwrap();
            assert!(gui.display(image).unwrap().display_visible());
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture("Demo_Controller", false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiControllers::new([("c1", 1), ("c2", 1), ("missing", 5)]));
    for _ in 0..4 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset, "Basics");
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(
        app.world().get::<GuiControllers>(owner),
        app.world().get::<GuiControllers>(view)
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiControllers>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiControllers>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn component_edits_preserve_indirect_controller_changes_and_native_gear_tweens() {
    let (mut app, owner, _) = fixture("Demo_Controller", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let button = child(&mut app, owner, "n11");
    let button_object = app.world().get::<GObjectRef>(button).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .click_button(button_object);
    app.world_mut()
        .get_mut::<GuiControllers>(view)
        .unwrap()
        .get_mut("c2")
        .unwrap()
        .selected_index = 1;
    app.update();
    assert_eq!((index(&app, view, "c1"), index(&app, view, "c2")), (1, 1));
    assert!(app.world().get::<GuiButtonSelected>(button).unwrap().0);
    let object = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let graph = child(&mut app, owner, "n3");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .advance_gear_tweens(10.);
    app.update();
    let transform = app.world().get::<GuiTransform>(graph).unwrap();
    assert_eq!(transform.position, Vec2::new(60., 328.));
    assert_eq!(transform.size, Vec2::new(196., 174.));
    // A later direct native page change must be published, never undone by a
    // component whose c1 field was untouched during this update.
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_controller_page_index(object, "c1", 0);
    app.world_mut()
        .get_mut::<GuiControllers>(view)
        .unwrap()
        .get_mut("c2")
        .unwrap()
        .selected_index = 0;
    app.update();
    assert_eq!((index(&app, view, "c1"), index(&app, view, "c2")), (0, 0));
    assert_eq!(
        app.world()
            .get::<GuiControllers>(view)
            .unwrap()
            .get("c1")
            .unwrap()
            .previous_index(),
        1
    );
}

#[test]
fn page_name_and_id_selection_preserve_metadata_and_normalize_invalid_indices() {
    let (mut app, owner, _) = fixture("Button4", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let original = app
        .world()
        .get::<GuiControllers>(view)
        .unwrap()
        .get("button")
        .unwrap()
        .clone();
    assert!(
        app.world_mut()
            .get_mut::<GuiControllers>(view)
            .unwrap()
            .get_mut("button")
            .unwrap()
            .set_selected_page_name("down")
    );
    app.update();
    assert_eq!(index(&app, view, "button"), 1);
    assert!(
        app.world_mut()
            .get_mut::<GuiControllers>(view)
            .unwrap()
            .get_mut("button")
            .unwrap()
            .set_selected_page_name("missing")
    );
    app.update();
    assert_eq!(index(&app, view, "button"), 0);
    // Unknown names follow the native fallback, including previous-page state.
    assert_eq!(
        app.world()
            .get::<GuiControllers>(view)
            .unwrap()
            .get("button")
            .unwrap()
            .previous_index(),
        1
    );
    let id = original.page_ids()[2].clone();
    app.world_mut()
        .get_mut::<GuiControllers>(view)
        .unwrap()
        .get_mut("button")
        .unwrap()
        .set_selected_page_id(&id);
    app.update();
    assert_eq!(index(&app, view, "button"), 2);
    app.world_mut()
        .get_mut::<GuiControllers>(view)
        .unwrap()
        .get_mut("button")
        .unwrap()
        .set_selected_page_id("absent");
    app.update();
    assert_eq!(index(&app, view, "button"), -1);
    for invalid in [i32::MAX, -2] {
        app.world_mut()
            .get_mut::<GuiControllers>(view)
            .unwrap()
            .get_mut("button")
            .unwrap()
            .selected_index = invalid;
        app.update();
        assert_eq!(index(&app, view, "button"), -1);
        assert_eq!(
            app.world()
                .get::<GuiControllers>(view)
                .unwrap()
                .get("button")
                .unwrap()
                .page_ids(),
            original.page_ids()
        );
    }
}

#[test]
fn initialize_and_post_layout_writes_reach_gears_in_the_same_frame() {
    fn initialize(
        roots: Query<&GuiRootView, Added<GuiRootView>>,
        mut controllers: Query<&mut GuiControllers>,
    ) {
        for view in &roots {
            controllers
                .get_mut(view.0)
                .unwrap()
                .get_mut("c1")
                .unwrap()
                .selected_index = 1;
        }
    }
    fn after_layout(
        roots: Query<&GuiRootView, Added<GuiRootView>>,
        mut controllers: Query<&mut GuiControllers>,
    ) {
        for view in &roots {
            let mut states = controllers.get_mut(view.0).unwrap();
            assert_eq!(states.get("c1").unwrap().selected_index, 1);
            states.get_mut("c2").unwrap().selected_index = 1;
        }
    }
    let (mut app, owner, asset) = fixture("Demo_Controller", false);
    app.add_systems(Update, initialize.in_set(GuiSystems::Initialize))
        .add_systems(Update, after_layout.in_set(GuiSystems::PostLayout));
    load(&mut app, &asset, "Basics");
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!((index(&app, view, "c1"), index(&app, view, "c2")), (1, 1));
    let ellipse = child(&mut app, owner, "n18");
    assert!(
        app.world()
            .get::<GuiDisplay>(ellipse)
            .unwrap()
            .display_visible()
    );
    assert_eq!(
        app.world().get::<Visibility>(ellipse),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn reparented_list_keeps_exported_controller_identity_through_settings_edits_and_owner_disposal() {
    let (mut app, owner, asset) = fixture("BagWin", false);
    load(&mut app, &asset, "Bag");
    let other = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "BagWin"))
        .id();
    app.update();
    let original_view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let other_view = app.world().get::<GuiRootView>(other).unwrap().0;
    for root in [owner, other] {
        let list = child(&mut app, root, "list");
        app.world_mut()
            .entity_mut(list)
            .insert(GuiListContent::new(45, |i, row| {
                row.set_text(&i.to_string())
            }));
    }
    app.update();
    let pages = child(&mut app, owner, "n25");
    let reference = app.world().get::<GObjectRef>(pages).unwrap().0;
    let parent = app.world().get::<GObjectRef>(other_view).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(reference, Some(parent));
    // Editing another field must not resolve the retained controller name
    // against the new parent's unrelated, same-name controller.
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .fold_invisible_items = true;
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![2];
    app.update();
    assert_eq!(app.world().get::<GObjectRef>(pages).unwrap().0, reference);
    assert_eq!(
        app.world().get::<GuiObjectInfo>(pages).unwrap().parent(),
        Some(other_view)
    );
    assert_eq!(index(&app, original_view, "page"), 2);
    assert_eq!(index(&app, other_view, "page"), 0);
    app.world_mut()
        .get_mut::<GuiControllers>(other_view)
        .unwrap()
        .get_mut("page")
        .unwrap()
        .selected_index = 1;
    app.update();
    assert_eq!(app.world().get::<GuiListSelection>(pages).unwrap().0, [2]);
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        app.world().get_entity(pages).is_ok(),
        "the moved list belongs to its current parent"
    );
    assert_eq!(
        app.world()
            .get::<GuiListSettings>(pages)
            .unwrap()
            .selection_controller,
        None
    );
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .selection_mode = ListSelectionMode::Multiple;
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![0];
    app.update();
    assert_eq!(
        index(&app, other_view, "page"),
        1,
        "a disposed controller must not retarget the current parent"
    );
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .selection_controller = Some("page".into());
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![2];
    app.update();
    assert_eq!(index(&app, other_view, "page"), 2);
    app.world_mut().despawn(other);
    app.update();
    assert!(app.world().get_entity(pages).is_err());
}

#[test]
fn list_settings_bind_selection_controllers_without_reselecting_and_merge_native_changes() {
    let (mut app, owner, asset) = fixture("BagWin", false);
    load(&mut app, &asset, "Bag");
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let list = child(&mut app, owner, "list");
    let pages = child(&mut app, owner, "n25");
    app.world_mut()
        .entity_mut(list)
        .insert(GuiListContent::new(45, |i, row| {
            row.set_text(&i.to_string())
        }));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListSettings>(pages)
            .unwrap()
            .selection_controller
            .as_deref(),
        Some("page")
    );
    {
        let mut settings = app.world_mut().get_mut::<GuiListSettings>(pages).unwrap();
        settings.selection_controller = None;
        settings.selection_mode = ListSelectionMode::Multiple;
    }
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![0, 2];
    app.update();
    assert_eq!(
        app.world().get::<GuiListSelection>(pages).unwrap().0,
        [0, 2],
        "mode must apply before same-frame selection"
    );
    assert_eq!(index(&app, view, "page"), 0);
    assert_eq!(
        app.world().get::<GuiScrollState>(list).unwrap().position(),
        Vec2::ZERO
    );
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .selection_controller = Some("page".into());
    app.update();
    assert_eq!(
        app.world().get::<GuiListSelection>(pages).unwrap().0,
        [0, 2],
        "binding alone must not reselect"
    );
    assert_eq!(index(&app, view, "page"), 0);
    {
        let mut settings = app.world_mut().get_mut::<GuiListSettings>(pages).unwrap();
        settings.selection_controller = Some("missing".into());
        settings.selection_mode = ListSelectionMode::Single;
    }
    app.update();
    let settings = app.world().get::<GuiListSettings>(pages).unwrap();
    assert_eq!(settings.selection_controller.as_deref(), Some("page"));
    assert_eq!(settings.selection_mode, ListSelectionMode::Single);
    assert_eq!(
        app.world().get::<GuiListSelection>(pages).unwrap().0,
        [0, 2],
        "mode assignment preserves an existing multi-selection"
    );

    let native = app.world().get::<GObjectRef>(pages).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_controller(native, None);
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .selection_mode = ListSelectionMode::MultipleSingleClick;
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListSettings>(pages)
            .unwrap()
            .selection_controller,
        None,
        "an unrelated component edit must retain the native unbind"
    );
    app.world_mut()
        .get_mut::<GuiListSettings>(pages)
        .unwrap()
        .selection_controller = Some("page".into());
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![1];
    app.update();
    assert_eq!(index(&app, view, "page"), 1);
    let pane = app.world().get::<GuiScrollState>(list).unwrap();
    assert_eq!(pane.position().x, pane.view_size().x);
    let tick = app
        .world()
        .entity(pages)
        .get_ref::<GuiListSettings>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(pages)
            .get_ref::<GuiListSettings>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn controller_components_drive_scroll_pages_and_selection_then_follow_native_scroll_back() {
    let (mut app, owner, asset) = fixture("BagWin", false);
    load(&mut app, &asset, "Bag");
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let list = child(&mut app, owner, "list");
    let pages = child(&mut app, owner, "n25");
    app.world_mut()
        .entity_mut(list)
        .insert(GuiListContent::new(45, |i, row| {
            row.set_text(&i.to_string());
        }));
    app.update();
    app.world_mut()
        .get_mut::<GuiControllers>(view)
        .unwrap()
        .get_mut("page")
        .unwrap()
        .selected_index = 2;
    app.update();
    let pane = app.world().get::<GuiScrollState>(list).unwrap();
    assert_eq!(pane.position().x, pane.view_size().x * 2.);
    assert_eq!(app.world().get::<GuiListSelection>(pages).unwrap().0, [2]);
    app.world_mut()
        .get_mut::<GuiListSelection>(pages)
        .unwrap()
        .0 = vec![1];
    app.update();
    assert_eq!(index(&app, view, "page"), 1);
    let pane = app.world().get::<GuiScrollState>(list).unwrap();
    assert_eq!(pane.position().x, pane.view_size().x);
    app.world_mut()
        .gui_mut(list)
        .unwrap()
        .scroll_to(Vec2::ZERO, false);
    app.update();
    assert_eq!(index(&app, view, "page"), 0);
    assert_eq!(app.world().get::<GuiListSelection>(pages).unwrap().0, [0]);
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}
