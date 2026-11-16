use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_asset::<Font>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        ));
    app.insert_resource(GuiFont(Some(font)));
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "Label"))
        .id();
    if loaded {
        load(&mut app, &handle);
        app.update();
    }
    (app, owner, handle)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "TextTemplate",
                    include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
                )
                .unwrap(),
                res_key: "ui/TextTemplate".into(),
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
fn span_entities(world: &World, entity: Entity) -> Vec<Entity> {
    let mut result = Vec::new();
    if let Some(children) = world.get::<Children>(entity) {
        for entity in children.iter() {
            if world.get::<TextSpan>(entity).is_some() {
                result.push(entity);
            }
            result.extend(span_entities(world, entity));
        }
    }
    result
}
fn text(world: &World, entity: Entity) -> String {
    let mut value = world
        .get::<Text>(entity)
        .map_or(String::new(), |text| text.0.clone());
    for span in span_entities(world, entity) {
        value.push_str(&world.get::<TextSpan>(span).unwrap().0);
    }
    value
}

#[test]
fn owner_options_wait_for_assets_and_reparse_nested_links_without_replacing_the_view() {
    let (mut app, owner, handle) = fixture(false);
    const SOURCE: &str = "  <a href='plain'>A</a>  <font color='#00FF00'><a href='explicit'>B</a></font>  <u><a href='underlined'>C</a></u>  ";
    let options = GuiHtmlOptions {
        link_color: Some("red".into()),
        link_underline: false,
        ignore_whitespace: true,
    };
    app.world_mut().entity_mut(owner).insert((
        GuiText::new(SOURCE),
        GuiTextFormat {
            html_enabled: true,
            font_size: 24,
            auto_size: 1,
            ..default()
        },
        options.clone(),
    ));
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    load(&mut app, &handle);
    app.update();
    let title = child(&mut app, owner, "title");
    assert_eq!(text(app.world(), title), "ABC");
    let spans = span_entities(app.world(), title);
    assert_eq!(spans.len(), 3);
    for (index, expected) in [
        Color::srgb(1., 0., 0.),
        Color::srgb(0., 1., 0.),
        Color::srgb(1., 0., 0.),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            app.world().get::<TextColor>(spans[index]).unwrap().0,
            expected
        );
        assert_eq!(
            app.world().get::<Underline>(spans[index]).is_some(),
            index == 2
        );
    }
    assert_eq!(app.world().get::<GuiHtmlOptions>(owner), Some(&options));
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        query.get(app.world()).unwrap().html_options(owner),
        Some(&options)
    );
    let original_width = app.world().get::<GuiTransform>(title).unwrap().size().x;
    app.world_mut()
        .get_mut::<GuiHtmlOptions>(owner)
        .unwrap()
        .ignore_whitespace = false;
    app.update();
    assert_eq!(text(app.world(), title), "  A  B  C  ");
    assert!(app.world().get::<GuiTransform>(title).unwrap().size().x > original_width);
    assert!(
        spans
            .iter()
            .all(|&span| app.world().get_entity(span).is_err())
    );
    assert_eq!(child(&mut app, owner, "title"), title);
    assert_eq!(app.world().get::<GuiText>(owner).unwrap().value(), SOURCE);
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_html_options(GuiHtmlOptions::default());
    app.update();
    let spans = span_entities(app.world(), title);
    for span in spans {
        let value = &app.world().get::<TextSpan>(span).unwrap().0;
        if ["A", "B", "C"].contains(&value.as_str()) {
            assert!(app.world().get::<Underline>(span).is_some());
        }
    }
    assert_eq!(
        app.world().get::<GuiHtmlOptions>(owner),
        Some(&GuiHtmlOptions::default())
    );
}

#[test]
fn options_merge_independent_native_edits_and_reject_unsupported_objects() {
    use crate::properties::sync_property;
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let object = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Text);
    let entity = world.spawn(GObjectRef(object)).id();
    sync_property::<GuiHtmlOptions>(&mut world);
    world
        .get_mut::<GuiHtmlOptions>(entity)
        .unwrap()
        .link_underline = false;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_html_options(
            object,
            crate::core::text::HtmlParseOptions {
                link_color: Some("blue".into()),
                ignore_whitespace: true,
                ..default()
            },
        );
    sync_property::<GuiHtmlOptions>(&mut world);
    assert_eq!(
        world.get::<GuiHtmlOptions>(entity),
        Some(&GuiHtmlOptions {
            link_underline: false,
            link_color: Some("blue".into()),
            ignore_whitespace: true,
        })
    );
    let image = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Image);
    let image_entity = world
        .spawn((GObjectRef(image), GuiHtmlOptions::default()))
        .id();
    sync_property::<GuiHtmlOptions>(&mut world);
    assert!(world.get::<GuiHtmlOptions>(image_entity).is_none());
    assert!(
        !world
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_html_options(image, default())
    );
}

#[test]
fn options_stay_local_and_survive_plain_text_and_ubb_backend_switches() {
    let (mut app, owner, handle) = fixture(true);
    let title = child(&mut app, owner, "title");
    let other = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(handle, "Label"),
            GuiText::new("[url=other]Other[/url]"),
            GuiTextFormat {
                ubb_enabled: true,
                ..default()
            },
        ))
        .id();
    app.world_mut().entity_mut(owner).insert((
        GuiText::new("[url=local]Local[/url]"),
        GuiHtmlOptions {
            link_color: Some("#11223380".into()),
            link_underline: false,
            ..default()
        },
    ));
    app.update();
    assert_eq!(text(app.world(), title), "[url=local]Local[/url]");
    app.world_mut()
        .get_mut::<GuiTextFormat>(title)
        .unwrap()
        .ubb_enabled = true;
    app.update();
    assert_eq!(text(app.world(), title), "Local");
    let span = span_entities(app.world(), title)[0];
    let color = app.world().get::<TextColor>(span).unwrap().0.to_srgba();
    assert!((color.alpha - 128. / 255.).abs() < 1e-6);
    assert!(app.world().get::<Underline>(span).is_none());
    let other_title = child(&mut app, other, "title");
    let other_span = span_entities(app.world(), other_title)[0];
    assert!(app.world().get::<Underline>(other_span).is_some());
    assert_eq!(
        app.world().get::<GuiHtmlOptions>(other_title),
        Some(&GuiHtmlOptions::default())
    );
}
