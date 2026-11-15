//! Compile and exercise build-generated bindings against real materialized ECS.
use bevy::{ecs::system::SystemState, prelude::*};
use fairygui_bevy::{binding::kind, prelude::*};

use crate::ui::{basics, pull_to_refresh};

fn fixture() -> (App, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .register_gui_extension::<basics::DemoWindow>();
    let package =
        fairygui_bevy::core::FuiPackage::parse("Basics", include_bytes!("../assets/ui/Basics.fui"))
            .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Basics".into(),
        });
    (app, asset)
}

#[test]
fn generated_binding_resolves_owner_nested_controls_and_auto_extensions_and_rejects_wrong_resources()
 {
    let (mut app, asset) = fixture();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(
            asset.clone(),
            basics::DemoWindow::ITEM_NAME,
        ))
        .id();
    let other = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), basics::Button2::ITEM_NAME))
        .id();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert!(matches!(
        basics::DemoWindow::bind(&query.get(app.world()).unwrap(), owner),
        Err(GuiBindingError::NotReady(_))
    ));
    app.update();
    app.update();
    let gui = query.get(app.world()).unwrap();
    let binding = basics::DemoWindow::bind(&gui, owner).unwrap();
    let view = gui.view(owner).unwrap();
    assert_eq!(binding.root.entity(), view);
    assert_eq!(
        binding.root,
        basics::DemoWindow::bind(&gui, owner).unwrap().root
    );
    let mut handles = std::collections::HashSet::new();
    handles.insert(binding.n0);
    assert!(handles.contains(&binding.n0));
    assert_eq!(binding.n0.entity(), gui.child_by_name(owner, "n0").unwrap());
    assert_eq!(
        app.world()
            .get::<basics::DemoWindow>(view)
            .unwrap()
            .n1
            .entity(),
        binding.n1.entity()
    );
    assert!(
        app.world()
            .get::<GuiExtensionState>(view)
            .unwrap()
            .error()
            .is_none()
    );
    let button = binding.n0.bind(&gui).unwrap();
    assert_eq!(
        gui.get(button.title.entity()).unwrap().kind(),
        fairygui_bevy::core::package::ObjectType::Text
    );
    assert!(matches!(
        GuiHandle::<kind::Text>::new(&gui, binding.n0.entity()),
        Err(GuiBindingError::WrongKind { .. })
    ));
    assert!(matches!(
        basics::Button1::bind(&gui, other),
        Err(GuiBindingError::WrongResource { .. })
    ));
    assert!(matches!(
        GuiHandle::<kind::Button>::child(&gui, view, "missing"),
        Err(GuiBindingError::MissingChild { .. })
    ));

    app.world_mut()
        .get_mut::<GuiText>(button.title.entity())
        .unwrap()
        .set("Typed title");
    app.update();
    assert_eq!(
        query.get(app.world()).unwrap().text(button.title.entity()),
        Some("Typed title")
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = basics::Button1::ITEM_NAME.into();
    app.update();
    assert!(
        binding
            .root
            .validate(&query.get(app.world()).unwrap())
            .is_err()
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        button
            .title
            .validate(&query.get(app.world()).unwrap())
            .is_err()
    );
}

#[test]
fn generated_controller_pages_edit_ecs_and_transitions_play_without_string_lookups() {
    let (mut app, asset) = fixture();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, basics::Button1::ITEM_NAME))
        .id();
    let package = fairygui_bevy::core::FuiPackage::parse(
        "PullToRefresh",
        include_bytes!("../assets/ui/PullToRefresh.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/PullToRefresh".into(),
        });
    let header = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, pull_to_refresh::Header::ITEM_NAME))
        .id();
    app.update();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let button = basics::Button1::bind(&query.get(app.world()).unwrap(), owner).unwrap();
    assert_eq!(
        button.c_button.selected(&query.get(app.world()).unwrap()),
        Some(basics::Button1ButtonPage::Up)
    );
    let mut controllers = SystemState::<Query<&mut GuiControllers>>::new(app.world_mut());
    assert!(
        button
            .c_button
            .select(
                &mut controllers.get_mut(app.world_mut()).unwrap(),
                basics::Button1ButtonPage::Down
            )
            .unwrap()
    );
    controllers.apply(app.world_mut());
    app.update();
    assert_eq!(
        button.c_button.selected(&query.get(app.world()).unwrap()),
        Some(basics::Button1ButtonPage::Down)
    );
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .controller(owner, "button")
            .unwrap()
            .selected_page_id(),
        Some(basics::Button1ButtonPage::Down.id())
    );
    let binding = pull_to_refresh::Header::bind(&query.get(app.world()).unwrap(), header).unwrap();
    let mut edit = SystemState::<GuiMut>::new(app.world_mut());
    assert!(
        binding
            .t_t0
            .play(&mut edit.get_mut(app.world_mut()).unwrap(), 1)
    );
    edit.apply(app.world_mut());
    app.update();
    assert!(
        binding
            .t_t0
            .state(&query.get(app.world()).unwrap())
            .unwrap()
            .is_playing()
    );

    // A controller with the right name but different exported pages must fail
    // before any generated enum can change it.
    #[derive(Debug, Clone, Copy)]
    struct WrongPage;
    impl GuiPage for WrongPage {
        const PAGES: &'static [(&'static str, &'static str)] = &[("wrong", "wrong")];
        fn id(self) -> &'static str {
            "wrong"
        }
        fn from_id(_: &str) -> Option<Self> {
            None
        }
    }
    assert!(matches!(
        GuiController::<WrongPage>::new(&query.get(app.world()).unwrap(), owner, "button"),
        Err(GuiBindingError::ControllerSchema { .. })
    ));
    app.world_mut().despawn(header);
    app.update();
    assert!(
        !binding
            .t_t0
            .play(&mut edit.get_mut(app.world_mut()).unwrap(), 1)
    );
}
