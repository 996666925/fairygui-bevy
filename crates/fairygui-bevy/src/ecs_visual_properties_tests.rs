use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use fairygui_core::{color::Color as NativeColor, object::GObjectId};

fn fixture(package: &str, item: &str, loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
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
        load(&mut app, &asset, package);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, asset: &Handle<FuiAsset>, package: &str) {
    let bytes: &[u8] = match package {
        "Basics" => include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
        "Cooldown" => include_bytes!("../../fairygui-demo/assets/ui/Cooldown.fui"),
        _ => unreachable!(),
    };
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(package, bytes).unwrap(),
                res_key: format!("ui/{package}"),
            },
        )
        .unwrap();
}
fn child(app: &mut App, parent: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(parent, name)
        .unwrap()
}
fn object(app: &App, view: Entity) -> GObjectId {
    app.world().get::<GObjectRef>(view).unwrap().0
}

#[test]
fn delayed_image_owner_applies_color_and_mask_before_initialize_without_changing_bounds() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.image_fill(owner).unwrap().amount, 0.25);
            assert_eq!(gui.color(owner), Some(Color::srgb(0.2, 0.6, 1.)));
            assert_eq!(gui.transform(owner).unwrap().size, Vec2::new(200., 100.));
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture("Basics", "pic", false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut().entity_mut(owner).insert((
        GuiColor(Color::srgb(0.2, 0.6, 1.)),
        GuiImageFill {
            method: FillMethod::Horizontal,
            amount: 0.25,
            ..default()
        },
        GuiTransform {
            size: Vec2::new(200., 100.),
            ..default()
        },
    ));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset, "Basics");
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!((node.width, node.height), (Val::Px(200.), Val::Px(100.)));
    // The material waits for a decoded texture. Supply the real exported atlas
    // after the initial view to cover that independent asynchronous boundary.
    let texture = app.world().get::<ImageNode>(view).unwrap().image.clone();
    let atlas = Image::from_buffer(
        include_bytes!("../../fairygui-demo/assets/ui/Basics_atlas0.png"),
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        default(),
        default(),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(texture.id(), atlas)
        .unwrap();
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<crate::image_fill::FillMaterial>>(view)
            .is_some()
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiImageFill>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiImageFill>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert_eq!(
        app.world().get::<GuiImageFill>(owner),
        app.world().get::<GuiImageFill>(view)
    );
}

#[test]
fn loader_settings_resize_component_relations_keep_identity_and_follow_native_autosize_feedback() {
    let (mut app, owner, _) = fixture("Basics", "Demo_Loader", true);
    let loader = child(&mut app, owner, "n2");
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://Basics/Component12");
    *app.world_mut()
        .get_mut::<GuiLoaderSettings>(loader)
        .unwrap() = GuiLoaderSettings {
        fill: 4,
        align: 2,
        vertical_align: 2,
        use_resize: true,
        show_error_sign: false,
        ..default()
    };
    app.world_mut()
        .get_mut::<GuiTransform>(loader)
        .unwrap()
        .size = Vec2::new(200., 150.);
    app.update();
    let loader_id = object(&app, loader);
    let runtime = app.world().non_send::<GuiRuntime>();
    let content = runtime.loader_component(loader_id).unwrap();
    let content_view = runtime.entity_of(content).unwrap();
    let background = runtime.store().child_by_name(content, "n9").unwrap();
    assert_eq!(
        runtime.store().object(content).unwrap().size(),
        crate::core::Size::new(200., 150.)
    );
    assert_eq!(
        runtime.store().object(background).unwrap().size(),
        crate::core::Size::new(200., 150.)
    );
    assert_eq!(
        app.world().get::<ChildOf>(content_view).unwrap().parent(),
        loader
    );
    assert_eq!(
        app.world().get::<GuiLoaderState>(loader),
        Some(&GuiLoaderState::Ready)
    );
    app.world_mut()
        .get_mut::<GuiLoaderSettings>(loader)
        .unwrap()
        .auto_size = true;
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(runtime.loader_component(loader_id), Some(content));
    assert_eq!(runtime.entity_of(content), Some(content_view));
    assert_eq!(
        app.world().get::<GuiTransform>(loader).unwrap().size,
        Vec2::new(400., 297.)
    );
    assert_eq!(
        runtime.store().object(content).unwrap().size(),
        crate::core::Size::new(400., 297.)
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(content_view).is_err());
}

#[test]
fn loader_field_edits_merge_with_native_changes_and_keep_settings_across_url_replacements() {
    let (mut app, owner, _) = fixture("Basics", "Demo_Loader", true);
    let loader = child(&mut app, owner, "n2");
    let loader_id = object(&app, loader);
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_loader_layout(loader_id, 5, 1, 1);
        runtime.store_mut().set_loader_shrink_only(loader_id, true);
    }
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiLoaderSettings>(loader)
            .unwrap();
        settings.align = 2;
        settings.show_error_sign = false;
    }
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://Missing/Resource");
    app.update();
    let expected = GuiLoaderSettings {
        fill: 5,
        align: 2,
        vertical_align: 1,
        shrink_only: true,
        show_error_sign: false,
        ..default()
    };
    assert_eq!(
        app.world().get::<GuiLoaderSettings>(loader),
        Some(&expected)
    );
    assert_eq!(
        app.world().get::<GuiLoaderState>(loader),
        Some(&GuiLoaderState::Failed(GuiLoaderFailure::MissingResource))
    );
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://Basics/Component12");
    app.update();
    assert_eq!(
        app.world().get::<GuiLoaderSettings>(loader),
        Some(&expected)
    );
    assert_eq!(
        app.world().get::<GuiLoaderState>(loader),
        Some(&GuiLoaderState::Ready)
    );
    app.world_mut().get_mut::<GuiIcon>(loader).unwrap().set("");
    app.update();
    assert_eq!(
        app.world().get::<GuiLoaderState>(loader),
        Some(&GuiLoaderState::Empty)
    );
    assert_eq!(
        app.world().get::<GuiLoaderSettings>(loader),
        Some(&expected)
    );
}

#[test]
fn progress_and_mask_component_edits_merge_and_retain_f64_amounts_and_layout() {
    let (mut app, owner, _) = fixture("Cooldown", "Main", true);
    let progress = child(&mut app, owner, "b0");
    let bar = child(&mut app, progress, "bar");
    let size = app.world().get::<GuiTransform>(bar).unwrap().size;
    app.world_mut()
        .get_mut::<GuiProgressValue>(progress)
        .unwrap()
        .0 = 70.;
    app.world_mut().get_mut::<GuiImageFill>(bar).unwrap().origin = 2;
    app.update();
    let fill = app.world().get::<GuiImageFill>(bar).unwrap();
    assert!((fill.amount - 0.7).abs() < 1e-12);
    assert_eq!(fill.origin, 2);
    let id = object(&app, bar);
    let amount = 0.12345678901234568;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill_amount(id, amount);
    app.world_mut()
        .get_mut::<GuiImageFill>(bar)
        .unwrap()
        .clockwise = true;
    app.update();
    assert_eq!(app.world().get::<GuiImageFill>(bar).unwrap().amount, amount);
    let fill = GuiImageFill {
        method: FillMethod::Horizontal,
        amount: 1.75,
        ..default()
    };
    app.world_mut().gui_mut(bar).unwrap().set_image_fill(fill);
    app.update();
    assert_eq!(app.world().get::<GuiImageFill>(bar), Some(&fill));
    app.world_mut().get_mut::<GuiImageFill>(bar).unwrap().amount = f64::NAN;
    app.world_mut().get_mut::<GuiImageFill>(bar).unwrap().method = FillMethod::Vertical;
    app.update();
    let fill = app.world().get::<GuiImageFill>(bar).unwrap();
    assert_eq!((fill.method, fill.amount), (FillMethod::Vertical, 1.75));
    assert_eq!(app.world().get::<GuiTransform>(bar).unwrap().size, size);
}

#[test]
fn shared_color_aliases_follow_gears_and_text_format_without_replaying_old_values() {
    let (mut app, owner, _) = fixture("Basics", "Demo_Controller", true);
    let text = child(&mut app, owner, "n6");
    let root = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().get_mut::<GuiColor>(text).unwrap().0 = Color::srgb(1., 0., 0.);
    app.world_mut().get_mut::<GuiTextFormat>(text).unwrap().bold = true;
    app.update();
    assert_eq!(
        app.world().get::<GuiTextFormat>(text).unwrap().color,
        Color::srgb(1., 0., 0.)
    );
    assert!(app.world().get::<GuiTextFormat>(text).unwrap().bold);
    app.world_mut()
        .get_mut::<GuiControllers>(root)
        .unwrap()
        .get_mut("c1")
        .unwrap()
        .selected_index = 1;
    app.world_mut()
        .get_mut::<GuiTextFormat>(text)
        .unwrap()
        .italic = true;
    app.update();
    assert_eq!(
        app.world().get::<GuiColor>(text).unwrap().0,
        Color::srgb(0., 0., 0.)
    );
    app.world_mut()
        .get_mut::<GuiControllers>(root)
        .unwrap()
        .get_mut("c1")
        .unwrap()
        .selected_index = 0;
    app.update();
    assert_eq!(
        app.world().get::<GuiColor>(text).unwrap().0,
        Color::srgb(1., 0., 0.)
    );
    app.world_mut()
        .get_mut::<GuiTextFormat>(text)
        .unwrap()
        .color = Color::srgb(0., 0., 1.);
    app.update();
    assert_eq!(
        app.world().get::<GuiColor>(text).unwrap().0,
        Color::srgb(0., 0., 1.)
    );
    app.world_mut()
        .gui_mut(text)
        .unwrap()
        .set_color(Color::linear_rgb(0.3, 0.4, 0.5));
    app.update();
    let color = Color::linear_rgb(0.3, 0.4, 0.5).to_srgba();
    assert_eq!(
        app.world().get::<GuiColor>(text).unwrap().0,
        Color::Srgba(color)
    );
    let previous = *app.world().get::<GuiColor>(text).unwrap();
    app.world_mut().get_mut::<GuiColor>(text).unwrap().0 = Color::srgba(f32::NAN, 0., 0., 1.);
    app.update();
    assert_eq!(app.world().get::<GuiColor>(text), Some(&previous));
    app.world_mut().get_mut::<GuiColor>(text).unwrap().0 = Color::srgb(1., 0., 0.);
    app.world_mut()
        .get_mut::<GuiTextFormat>(text)
        .unwrap()
        .color = Color::srgb(0., 1., 0.);
    app.update();
    assert_eq!(
        app.world().get::<GuiColor>(text).unwrap().0,
        Color::srgb(0., 1., 0.)
    );
}

#[test]
fn color_routes_to_native_image_movie_loader_graph_and_titles_but_omits_plain_containers() {
    let (mut app, owner, _) = fixture("Basics", "Demo_Loader", true);
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let kinds = [
        crate::core::package::ObjectType::Image,
        crate::core::package::ObjectType::MovieClip,
        crate::core::package::ObjectType::Loader,
        crate::core::package::ObjectType::Graph,
        crate::core::package::ObjectType::Label,
        crate::core::package::ObjectType::Button,
        crate::core::package::ObjectType::ComboBox,
    ];
    let ids: Vec<_> = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        kinds
            .into_iter()
            .map(|kind| {
                let id = runtime.store_mut().create(kind);
                runtime.store_mut().set_parent(id, Some(root));
                if matches!(
                    kind,
                    crate::core::package::ObjectType::Label
                        | crate::core::package::ObjectType::Button
                        | crate::core::package::ObjectType::ComboBox
                ) {
                    let title = runtime
                        .store_mut()
                        .create(crate::core::package::ObjectType::Text);
                    runtime.store_mut().set_name(title, "title");
                    runtime.store_mut().set_parent(title, Some(id));
                }
                runtime.queue_subtree(id);
                id
            })
            .collect()
    };
    app.update();
    let views: Vec<_> = ids
        .iter()
        .map(|&id| app.world().non_send::<GuiRuntime>().entity_of(id).unwrap())
        .collect();
    for &view in &views {
        app.world_mut().get_mut::<GuiColor>(view).unwrap().0 = Color::srgb(0.2, 0.4, 0.6);
    }
    app.update();
    for &id in &ids {
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object_color(id),
            Some(NativeColor::new(0.2, 0.4, 0.6, 1.))
        );
    }
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert!(app.world().get::<GuiColor>(view).is_none());
    app.world_mut()
        .entity_mut(view)
        .insert((GuiImageFill::default(), GuiLoaderSettings::default()));
    app.update();
    assert!(app.world().get::<GuiImageFill>(view).is_none());
    assert!(app.world().get::<GuiLoaderSettings>(view).is_none());
}
