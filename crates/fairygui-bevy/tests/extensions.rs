use bevy::{ecs::system::EntityCommands, prelude::*};
use fairygui_bevy::prelude::*;

#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/mailItem", on_construct = attach)]
struct Mail {
    #[gui(path = "")]
    entity: Entity,
    #[gui(path = "timeText")]
    time: Entity,
    #[gui(path = "absent")]
    optional: Option<Entity>,
    clicks: usize,
}
fn attach(entity: &mut EntityCommands) {
    entity.observe(|event: On<GuiEvent>, mut items: Query<&mut Mail>| {
        if event.event.kind == fairygui_bevy::core::object::EventKind::Click {
            items.get_mut(event.entity).unwrap().clicks += 1;
        }
    });
    entity.insert(GuiDisplay::default().with_alpha(0.75));
}

#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/Main")]
struct Page {
    #[gui(path = "mailList")]
    list: Entity,
}

fn app() -> App {
    let mut app = App::new();
    // Registration before the plugin/package is allowed.
    app.register_gui_extension::<Mail>()
        .register_gui_extension::<Page>()
        .add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            FairyGuiPlugin,
        ))
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<bevy::input::mouse::MouseWheel>();
    app
}
fn asset(app: &mut App) -> Handle<FuiAsset> {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_bevy::core::FuiPackage::parse(
                "VirtualList",
                include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
            )
            .unwrap(),
            res_key: "ui/VirtualList".into(),
        })
}
fn configure(mut commands: Commands, pages: Query<&Page, Added<Page>>) {
    for page in &pages {
        commands
            .entity(page.list)
            .insert(GuiVirtualList::new(1000, |index, row| {
                row.set_text(&format!("Mail {index}"));
                if let Some(mut time) = row.child_by_path("timeText") {
                    time.set_text("bound time");
                }
            }));
    }
}
fn rows(app: &App, list: Entity) -> Vec<Entity> {
    app.world()
        .get::<GuiListState>(list)
        .unwrap()
        .items()
        .to_vec()
}

#[test]
fn extensions_are_ready_for_initialize_and_post_layout_keep_bound_children_on_pool_reuse_and_dispose()
 {
    let mut app = app();
    app.add_systems(Update, configure.in_set(GuiSystems::Initialize));
    let handle = asset(&mut app);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    let root_view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let list = app
        .world()
        .get::<Page>(root_view)
        .expect("before Initialize")
        .list;
    assert_eq!(
        app.world().get::<GuiListState>(list).unwrap().item_count(),
        1000
    );
    let first = rows(&app, list);
    assert!(!first.is_empty());
    let bound: Vec<_> = first
        .iter()
        .map(|&row| {
            let mail = app
                .world()
                .get::<Mail>(row)
                .expect("before first PostLayout");
            assert_eq!(mail.entity, row);
            assert!(mail.optional.is_none());
            assert_eq!(mail.clicks, 0);
            assert_eq!(
                app.world().get::<GuiText>(mail.time).unwrap().value(),
                "bound time"
            );
            assert_eq!(app.world().get::<GuiDisplay>(row).unwrap().alpha(), 0.75);
            assert!(
                app.world()
                    .get::<GuiExtensionState>(row)
                    .unwrap()
                    .error()
                    .is_none()
            );
            (row, mail.time)
        })
        .collect();
    let tick = app
        .world()
        .entity(first[0])
        .get_ref::<GuiExtensionState>()
        .unwrap()
        .last_changed();
    let object = app.world().get::<GObjectRef>(first[0]).unwrap().0;
    app.world_mut().trigger(GuiEvent::new(
        first[0],
        fairygui_bevy::core::object::UiEvent::new(
            fairygui_bevy::core::object::EventKind::Click,
            object,
        ),
    ));
    app.update();
    assert_eq!(app.world().get::<Mail>(first[0]).unwrap().clicks, 1);
    assert_eq!(
        app.world()
            .entity(first[0])
            .get_ref::<GuiExtensionState>()
            .unwrap()
            .last_changed(),
        tick
    );
    if let Some(mut object) = app.world_mut().gui_mut(list) {
        object.scroll_to(Vec2::new(0., 15000.), false)
    }
    app.update();
    assert!(rows(&app, list).iter().any(|row| first.contains(row)));
    for (row, time) in &bound {
        assert_eq!(app.world().get::<Mail>(*row).unwrap().time, *time);
    }
    assert_eq!(
        app.world().get::<Mail>(first[0]).unwrap().clicks,
        1,
        "constructors must not rerun on rebind"
    );
    // Removing content pools rows without discarding their application state.
    app.world_mut().entity_mut(list).remove::<GuiVirtualList>();
    app.update();
    assert!(rows(&app, list).is_empty());
    assert!(
        bound
            .iter()
            .all(|(row, _)| app.world().get::<Mail>(*row).is_some())
    );
    app.world_mut().entity_mut(owner).despawn();
    app.update();
    for (row, time) in bound {
        assert!(app.world().get_entity(row).is_err());
        assert!(app.world().get_entity(time).is_err());
    }
}

#[derive(Component, GuiExtension)]
#[gui(url = "ui://qkteqwfpc8s24")]
struct ById;
#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/mailItem")]
struct Replacement;
#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/Main")]
struct Invalid {
    #[gui(path = "does/not/exist")]
    _child: Entity,
}

#[test]
fn id_precedence_late_registrations_replacement_and_binding_errors_preserve_existing_instances() {
    let mut app = app();
    app.register_gui_extension::<ById>();
    let handle = asset(&mut app);
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "mailItem"))
        .id();
    app.update();
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    assert!(app.world().get::<ById>(first_view).is_some());
    assert!(
        app.world().get::<Mail>(first_view).is_none(),
        "ID URL wins over package/name URL"
    );
    app.register_gui_extension_at::<Replacement>(ById::URL);
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "mailItem"))
        .id();
    app.update();
    let second_view = app.world().get::<GuiRootView>(second).unwrap().0;
    assert!(app.world().get::<Replacement>(second_view).is_some());
    assert!(app.world().get::<ById>(first_view).is_some());
    assert!(app.world().get::<Replacement>(first_view).is_none());
    app.register_gui_extension::<Invalid>();
    let root = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    let view = app.world().get::<GuiRootView>(root).unwrap().0;
    assert!(app.world().get::<Invalid>(view).is_none());
    let state = app.world().get::<GuiExtensionState>(view).unwrap();
    assert_eq!(
        state.error(),
        Some(&GuiExtensionError::MissingChild("does/not/exist".into()))
    );
    assert!(
        app.world().get::<GuiObjectInfo>(view).is_some(),
        "a bad extension does not destroy native content"
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiExtensionState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiExtensionState>()
            .unwrap()
            .last_changed(),
        tick
    );
    // Changing the root source creates a fresh control and runs the new factory.
    app.world_mut()
        .get_mut::<FairyGuiRoot>(root)
        .unwrap()
        .item_name = "mailItem".into();
    app.update();
    let replacement = app.world().get::<GuiRootView>(root).unwrap().0;
    assert_ne!(replacement, view);
    assert!(app.world().get::<Replacement>(replacement).is_some());
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn registering_after_materialization_applies_only_to_future_views() {
    let mut app = app();
    let handle = asset(&mut app);
    let mut inactive = FairyGuiRoot::new(handle.clone(), "mailItem");
    inactive.active = false;
    let first = app.world_mut().spawn(inactive).id();
    app.update();
    let view = app.world().get::<GuiRootView>(first).unwrap().0;
    app.register_gui_extension::<ById>();
    app.update();
    assert!(app.world().get::<ById>(view).is_none());
    assert!(app.world().get::<Mail>(view).is_some());
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "mailItem"))
        .id();
    app.update();
    assert!(
        app.world()
            .get::<ById>(app.world().get::<GuiRootView>(second).unwrap().0)
            .is_some()
    );
}

#[test]
fn extension_waits_for_pending_assets_and_binds_nested_cross_package_loader_content() {
    let mut app = app();
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(pending.clone(), "mailItem"))
        .id();
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    let package = fairygui_bevy::core::FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            pending.id(),
            FuiAsset {
                package,
                res_key: "ui/VirtualList".into(),
            },
        )
        .unwrap();
    app.update();
    assert!(
        app.world()
            .get::<Mail>(app.world().get::<GuiRootView>(owner).unwrap().0)
            .is_some()
    );
    let menu = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_bevy::core::FuiPackage::parse(
                "MainMenu",
                include_bytes!("../../fairygui-demo/assets/ui/MainMenu.fui"),
            )
            .unwrap(),
            res_key: "ui/MainMenu".into(),
        });
    let page = app.world_mut().spawn(FairyGuiRoot::new(menu, "Main")).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(page).unwrap().object;
    let loader = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let loader = runtime
            .store_mut()
            .create(fairygui_bevy::core::package::ObjectType::Loader);
        runtime.store_mut().set_parent(loader, Some(root));
        runtime.store_mut().set_size(loader, 400., 100.);
        runtime
            .store_mut()
            .apply_icon_to(loader, "ui://VirtualList/mailItem");
        runtime.queue_subtree(loader);
        loader
    };
    app.update();
    let (loader_view, child) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (
            runtime.entity_of(loader).unwrap(),
            runtime
                .entity_of(runtime.loader_component(loader).unwrap())
                .unwrap(),
        )
    };
    let mail = app
        .world()
        .get::<Mail>(child)
        .expect("loader-created component uses the same extension factory");
    let time = mail.time;
    assert_eq!(
        app.world()
            .get::<GuiExtensionState>(child)
            .unwrap()
            .resource_url(),
        "ui://qkteqwfpc8s24"
    );
    if let Some(mut object) = app.world_mut().gui_mut(loader_view) {
        object.set_icon(String::new())
    }
    app.update();
    assert!(app.world().get_entity(child).is_err());
    assert!(app.world().get_entity(time).is_err());
}
