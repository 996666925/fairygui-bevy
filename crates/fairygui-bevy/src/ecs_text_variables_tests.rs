use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture() -> (App, Entity) {
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
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "TextTemplate",
                include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
            )
            .unwrap(),
            res_key: "ui/TextTemplate".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    (app, owner)
}
fn child(app: &mut App, owner: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}
fn spans(world: &World, entity: Entity) -> String {
    let mut value = world
        .get::<TextSpan>(entity)
        .map_or(String::new(), |s| s.0.clone());
    if let Some(children) = world.get::<Children>(entity) {
        for child in children.iter() {
            value.push_str(&spans(world, child));
        }
    }
    value
}

#[test]
fn exported_template_defaults_expand_on_first_frame_and_variables_remeasure_without_changing_source()
 {
    let (mut app, owner) = fixture();
    let plain = child(&mut app, owner, "plain");
    let disabled = child(&mut app, owner, "disabled");
    assert_eq!(app.world().get::<Text>(plain).unwrap().0, "Hello Guest! ");
    assert_eq!(
        app.world().get::<GuiText>(plain).unwrap().value(),
        "Hello {name=Guest}! {missing}"
    );
    assert_eq!(
        app.world().get::<Text>(disabled).unwrap().0,
        "Literal {name=Guest}"
    );
    let width = app.world().get::<GuiTransform>(plain).unwrap().size.x;
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .set("name", "A much longer 中文 name");
    app.update();
    assert_eq!(
        app.world().get::<Text>(plain).unwrap().0,
        "Hello A much longer 中文 name! "
    );
    assert!(app.world().get::<GuiTransform>(plain).unwrap().size.x > width);
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .disable();
    app.update();
    assert_eq!(
        app.world().get::<Text>(plain).unwrap().0,
        "Hello {name=Guest}! {missing}"
    );
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .clear();
    app.update();
    assert_eq!(app.world().get::<Text>(plain).unwrap().0, "Hello Guest! ");
}

#[test]
fn nested_titles_rich_markup_and_inputs_receive_variables_and_keep_raw_values() {
    let (mut app, owner) = fixture();
    let nested = child(&mut app, owner, "nested");
    let title = child(&mut app, nested, "title");
    let rich = child(&mut app, owner, "rich");
    let input = child(&mut app, owner, "input");
    assert_eq!(spans(app.world(), rich), "Guest / none");
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .unwrap()
            .value(),
        "Guest"
    );
    app.world_mut()
        .entity_mut(nested)
        .insert(GuiTextVars::new([("name", "Nested 中文")]));
    app.world_mut().entity_mut(rich).insert(GuiTextVars::new([
        ("name", "[color=#ff0000]Red[/color]"),
        ("item", "<i>HTML</i>"),
    ]));
    app.world_mut()
        .entity_mut(input)
        .insert(GuiTextVars::new([("name", "Input text")]));
    app.update();
    assert_eq!(
        app.world().get::<Text>(title).unwrap().0,
        "Nested Nested 中文"
    );
    assert_eq!(spans(app.world(), rich), "Red / HTML");
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .unwrap()
            .value(),
        "Input text"
    );
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "{name=Guest}"
    );
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "{name=Guest}"
    );
    assert_eq!(child(&mut app, nested, "title"), title);
    app.world_mut()
        .gui_mut(nested)
        .unwrap()
        .set_text_variables(GuiTextVars::new([("name", "Immediate")]));
    app.update();
    assert_eq!(
        app.world().get::<Text>(title).unwrap().0,
        "Nested Immediate"
    );
}

#[test]
fn variable_component_merges_independent_native_edits_and_ignores_unsupported_controls() {
    use crate::properties::sync_property;
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let text = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Text);
    let entity = world.spawn(GObjectRef(text)).id();
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text_variables(text, GuiTextVars::new([("a", "1"), ("b", "2")]).0);
    sync_property::<GuiTextVars>(&mut world);
    world
        .get_mut::<GuiTextVars>(entity)
        .unwrap()
        .set("a", "new");
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text_variables(text, GuiTextVars::new([("a", "1"), ("b", "native")]).0);
    sync_property::<GuiTextVars>(&mut world);
    assert_eq!(
        world.get::<GuiTextVars>(entity),
        Some(&GuiTextVars::new([("a", "new"), ("b", "native")]))
    );
    let image = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Image);
    let image = world
        .spawn((GObjectRef(image), GuiTextVars::new([("a", "x")])))
        .id();
    sync_property::<GuiTextVars>(&mut world);
    assert!(world.get::<GuiTextVars>(image).is_none());
}

#[test]
fn bitmap_backend_expands_variables_before_layout_and_preserves_single_expansion() {
    let (mut app, owner) = fixture();
    let plain = child(&mut app, owner, "plain");
    let package = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let font = package
        .item(package.item_index_by_name("HitNumber").unwrap())
        .unwrap();
    let url = format!("ui://{}{}", package.id(), font.id);
    let _bitmap_asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Basics".into(),
        });
    app.world_mut()
        .get_mut::<GuiText>(plain)
        .unwrap()
        .set("{number=123}");
    app.world_mut()
        .get_mut::<GuiTextFormat>(plain)
        .unwrap()
        .font = Some(url);
    app.update();
    let original = app
        .world()
        .get::<crate::runtime::BitmapTextMetrics>(plain)
        .unwrap()
        .object_size;
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .set("number", "123456789");
    app.update();
    let next = app
        .world()
        .get::<crate::runtime::BitmapTextMetrics>(plain)
        .unwrap()
        .object_size;
    assert!(next.width > original.width, "{original:?} -> {next:?}");
    app.world_mut()
        .get_mut::<GuiTextFormat>(plain)
        .unwrap()
        .font = None;
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .set("number", "{name}");
    app.world_mut()
        .get_mut::<GuiTextVars>(plain)
        .unwrap()
        .set("name", "must not expand twice");
    app.update();
    assert_eq!(app.world().get::<Text>(plain).unwrap().0, "{name}");
}

#[test]
fn owner_variables_wait_for_assets_and_translation_precedes_expansion() {
    let (mut app, _) = fixture();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(handle.clone(), "Label"),
            GuiText::new("!welcome!"),
            GuiTextVars::new([("name", "中文")]),
        ))
        .id();
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    let mut table = crate::core::translation::TranslationTable::new();
    table.set("welcome", "Welcome {name}!");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_translations(table);
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
    app.update();
    let title = child(&mut app, owner, "title");
    assert_eq!(app.world().get::<Text>(title).unwrap().0, "Welcome 中文!");
    assert_eq!(
        app.world().get::<GuiText>(owner).unwrap().value(),
        "!welcome!"
    );
    let original = app.world().get::<GuiTransform>(title).unwrap().size;
    app.world_mut()
        .get_mut::<GuiTextVars>(owner)
        .unwrap()
        .set("name", "Much longer name 中文");
    app.update();
    assert_eq!(
        app.world().get::<Text>(title).unwrap().0,
        "Welcome Much longer name 中文!"
    );
    assert!(app.world().get::<GuiTransform>(title).unwrap().size.x > original.x);
}

#[test]
fn template_input_password_masks_expansion_and_user_edits_replace_the_template() {
    let (mut app, owner) = fixture();
    let input = child(&mut app, owner, "input");
    app.world_mut()
        .get_mut::<GuiTextVars>(input)
        .unwrap()
        .set("name", "😀中文");
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .password = true;
    app.update();
    assert_eq!(app.world().get::<Text>(input).unwrap().0, "●●●●");
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .unwrap()
            .value(),
        "😀中文"
    );
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "{name=Guest}"
    );
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(input)
        .unwrap()
        .editor
        .set_text("Edited");
    app.update();
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), "Edited");
    assert_eq!(app.world().get::<Text>(input).unwrap().0, "●●●●●●");
    app.world_mut().get_mut::<GuiText>(input).unwrap().set("");
    app.update();
    let prompt = app
        .world()
        .get::<Children>(input)
        .unwrap()
        .iter()
        .find_map(|entity| app.world().get::<Text>(entity))
        .unwrap();
    assert_eq!(prompt.0, "Prompt {name}");
}
