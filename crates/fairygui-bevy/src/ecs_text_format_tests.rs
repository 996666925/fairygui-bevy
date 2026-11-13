use crate::prelude::*;
use bevy::text::{LetterSpacing, LineHeight};
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(item: &str, loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
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
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), item))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
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

#[test]
fn owner_format_waits_for_assets_forwards_to_title_and_is_ready_at_initialize() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.text_format(owner).unwrap().font_size, 32);
            assert_eq!(
                gui.text_format(owner),
                gui.text_format(gui.child_by_name(owner, "title").unwrap())
            );
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture("Button1", false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut().entity_mut(owner).insert((
        GuiText::new("Hello"),
        GuiTextFormat {
            font_size: 32,
            color: Color::linear_rgb(0.25, 0.5, 0.75),
            align: 2,
            vertical_align: 2,
            bold: true,
            italic: true,
            leading: 9,
            letter_spacing: 3,
            single_line: true,
            ..default()
        },
    ));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let title = child(&mut app, owner, "title");
    let font = app.world().get::<TextFont>(title).unwrap();
    assert_eq!(font.font_size, FontSize::Px(32.));
    assert_eq!(font.weight, FontWeight::BOLD);
    assert_eq!(font.style, FontStyle::Italic);
    assert_eq!(
        *app.world().get::<LineHeight>(title).unwrap(),
        LineHeight::Px(41.)
    );
    assert_eq!(
        *app.world().get::<LetterSpacing>(title).unwrap(),
        LetterSpacing::Px(3.)
    );
    assert_eq!(
        app.world().get::<TextLayout>(title).unwrap().justify,
        Justify::Right
    );
    assert_eq!(app.world().get::<GuiText>(view).unwrap().value(), "Hello");
    let color = Color::linear_rgb(0.25, 0.5, 0.75).to_srgba();
    assert_eq!(
        app.world().get::<GuiTextFormat>(view).unwrap().color,
        Color::Srgba(color)
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiTextFormat>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiTextFormat>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert_eq!(
        app.world().get::<GuiTextFormat>(owner),
        app.world().get::<GuiTextFormat>(title)
    );
}

#[test]
fn controller_color_and_font_gears_merge_with_style_edits_using_native_capture_rules() {
    let (mut app, owner, _) = fixture("Demo_Controller", true);
    let root = app.world().get::<GuiRootView>(owner).unwrap().0;
    let color_text = child(&mut app, owner, "n6");
    let sized_text = child(&mut app, owner, "n17");
    let red = Color::srgb(1., 0., 0.);
    app.world_mut()
        .get_mut::<GuiTextFormat>(color_text)
        .unwrap()
        .color = red;
    app.world_mut()
        .get_mut::<GuiTextFormat>(sized_text)
        .unwrap()
        .font_size = 57;
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTextFormat>(sized_text)
            .unwrap()
            .font_size,
        57
    );
    app.world_mut()
        .get_mut::<GuiControllers>(root)
        .unwrap()
        .get_mut("c1")
        .unwrap()
        .selected_index = 1;
    app.world_mut()
        .get_mut::<GuiTextFormat>(color_text)
        .unwrap()
        .underline = true;
    app.world_mut()
        .get_mut::<GuiTextFormat>(sized_text)
        .unwrap()
        .bold = true;
    app.update();
    let color = app.world().get::<GuiTextFormat>(color_text).unwrap();
    assert_eq!(color.color, Color::srgb(0., 0., 0.));
    assert!(color.underline);
    let sized = app.world().get::<GuiTextFormat>(sized_text).unwrap();
    assert_eq!(sized.font_size, 74);
    assert!(sized.bold);
    app.world_mut()
        .get_mut::<GuiControllers>(root)
        .unwrap()
        .get_mut("c1")
        .unwrap()
        .selected_index = 0;
    app.update();
    assert_eq!(
        app.world().get::<GuiTextFormat>(color_text).unwrap().color,
        red
    );
    // Native fontSize writes do not capture GearFontSize, unlike color.
    assert_eq!(
        app.world()
            .get::<GuiTextFormat>(sized_text)
            .unwrap()
            .font_size,
        33
    );
}

#[test]
fn direct_model_changes_and_component_title_edits_merge_without_replaying_stale_fields() {
    let (mut app, owner, _) = fixture("Button1", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let title = child(&mut app, view, "title");
    let initial = app.world().get::<GuiTextFormat>(view).unwrap().clone();
    app.world_mut().entity_mut(owner).insert(initial);
    app.update();
    let mut native = app
        .world_mut()
        .gui_mut(owner)
        .unwrap()
        .text_format()
        .unwrap();
    native.font_size = 37;
    native.stroke_color = Color::srgb(0., 0.4, 0.6);
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_text_format(native);
    app.world_mut()
        .get_mut::<GuiTextFormat>(view)
        .unwrap()
        .stroke = 3.;
    app.world_mut()
        .get_mut::<GuiTextFormat>(title)
        .unwrap()
        .letter_spacing = 4;
    app.update();
    let format = app.world().get::<GuiTextFormat>(view).unwrap();
    assert_eq!(format.font_size, 37);
    assert_eq!(format.stroke, 3.);
    assert_eq!(format.stroke_color, Color::srgb(0., 0.4, 0.6));
    assert_eq!(format.letter_spacing, 4);
    assert_eq!(
        *app.world().get::<LetterSpacing>(title).unwrap(),
        LetterSpacing::Px(4.)
    );
    assert_eq!(app.world().get::<GuiTextFormat>(title), Some(format));
    assert_eq!(app.world().get::<GuiTextFormat>(owner), Some(format));
    // Removing a projection restores it from the model without resetting it.
    app.world_mut().entity_mut(title).remove::<GuiTextFormat>();
    app.update();
    assert_eq!(
        app.world().get::<GuiTextFormat>(title).unwrap().font_size,
        37
    );
    let saved = app.world().get::<GuiTextFormat>(title).unwrap().clone();
    {
        let mut format = app.world_mut().get_mut::<GuiTextFormat>(title).unwrap();
        format.stroke = f32::NAN;
        format.color = Color::srgba(f32::NAN, 0., 0., 1.);
        format.stroke_color = Color::linear_rgb(f32::INFINITY, 0., 0.);
    }
    app.update();
    assert_eq!(app.world().get::<GuiTextFormat>(title), Some(&saved));
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(title).is_err());
}

#[test]
fn same_frame_auto_size_tracks_format_text_and_delayed_font_measurement() {
    let (mut app, owner, _) = fixture("Demo_Text", true);
    let text = child(&mut app, owner, "n10");
    app.world_mut()
        .get_mut::<GuiText>(text)
        .unwrap()
        .set("Wide 中文 text");
    {
        let mut format = app.world_mut().get_mut::<GuiTextFormat>(text).unwrap();
        format.ubb_enabled = false;
        format.html_enabled = false;
        format.auto_size = 1;
        format.font_size = 20;
    }
    app.update();
    let before = app.world().get::<GuiTransform>(text).unwrap().size;
    assert!(before.x > 30. && before.y > 10.);
    let font = app.world().resource::<GuiFont>().0.clone().unwrap();
    let data = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .remove(font.id())
        .unwrap();
    app.world_mut()
        .get_mut::<GuiTextFormat>(text)
        .unwrap()
        .font_size = 40;
    app.update();
    assert_eq!(app.world().get::<GuiTransform>(text).unwrap().size, before);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(font.id(), data)
        .unwrap();
    app.update();
    let after = app.world().get::<GuiTransform>(text).unwrap().size;
    assert!(
        after.x > before.x * 1.7 && after.y > before.y * 1.7,
        "{before:?} -> {after:?}"
    );
    let node = app.world().get::<Node>(text).unwrap();
    assert_eq!(
        (node.width, node.height),
        (Val::Px(after.x), Val::Px(after.y))
    );
    app.world_mut().get_mut::<GuiText>(text).unwrap().set("");
    app.update();
    assert!(app.world().get::<GuiTransform>(text).unwrap().size.x < before.x);
}

#[test]
fn input_backend_switches_via_format_preserve_value_identity_and_application_children() {
    let (mut app, owner, _) = fixture("Demo_Text", true);
    let input = child(&mut app, owner, "n22");
    let application_child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(input)))
        .id();
    let package = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let item = package
        .item(package.item_index_by_name("HitNumber").unwrap())
        .unwrap();
    let font_url = format!("ui://{}{}", package.id(), item.id);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(handle.clone(), &package);
    app.world_mut()
        .get_mut::<GuiText>(input)
        .unwrap()
        .set("123");
    {
        let mut format = app.world_mut().get_mut::<GuiTextFormat>(input).unwrap();
        format.font = Some(font_url);
        format.ubb_enabled = false;
        format.html_enabled = false;
        format.underline = true;
        format.font_size = 36;
    }
    app.update();
    assert!(
        app.world()
            .get::<crate::runtime::BitmapGlyphs>(input)
            .is_some()
    );
    assert!(app.world().get::<bevy::text::EditableText>(input).is_none());
    assert!(app.world().get_entity(application_child).is_ok());
    app.world_mut()
        .get_mut::<GuiTextFormat>(input)
        .unwrap()
        .font = None;
    app.update();
    assert!(app.world().get::<bevy::text::EditableText>(input).is_some());
    assert_eq!(app.world().get::<Text>(input).unwrap().0, "123");
    app.world_mut()
        .get_mut::<GuiText>(input)
        .unwrap()
        .set("[b]Styled[/b]");
    app.world_mut()
        .get_mut::<GuiTextFormat>(input)
        .unwrap()
        .ubb_enabled = true;
    app.update();
    assert!(
        app.world()
            .get::<crate::runtime::RichTextBlock>(input)
            .is_some()
    );
    app.world_mut()
        .get_mut::<GuiTextFormat>(input)
        .unwrap()
        .ubb_enabled = false;
    app.update();
    assert!(app.world().get::<bevy::text::EditableText>(input).is_some());
    assert_eq!(
        app.world().get::<GuiText>(input).unwrap().value(),
        "[b]Styled[/b]"
    );
    assert!(app.world().get_entity(application_child).is_ok());
    drop(handle);
}
