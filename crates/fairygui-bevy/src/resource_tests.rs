use super::*;
const MAIN_MENU: &[u8] = include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui");

#[test]
fn exported_controller_roundtrip_restores_runtime_geometry_and_color_on_the_same_entities() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui
        .create_root(handle, &package, "Demo_Controller")
        .unwrap();
    let root = gui.root_object(index).unwrap();
    let graph = gui.store().child_by_name(root, "n3").unwrap();
    let image = gui.store().child_by_name(root, "n9").unwrap();
    let text = gui.store().child_by_name(root, "n6").unwrap();
    let font = gui.store().child_by_name(root, "n17").unwrap();
    let original_font = gui
        .store()
        .object(font)
        .unwrap()
        .control()
        .as_text_field()
        .unwrap();
    let original_size = original_font.font_size;
    let original_line_height = backend::text_line_height_for(original_font, None);
    let blue = fairygui_core::color::Color::new(0.1, 0.4, 0.8, 1.0);
    let orange = fairygui_core::color::Color::new(0.8, 0.4, 0.1, 1.0);
    gui.store_mut().set_xy(graph, 210.0, 290.0);
    gui.store_mut().set_size(graph, 130.0, 70.0);
    gui.store_mut().set_control_color(image, blue, None);
    gui.store_mut().set_control_color(text, orange, None);
    gui.store_mut()
        .set_object_font_size(font, original_size + 14);
    app.insert_non_send(gui).add_systems(
        Update,
        (spawn_pending_roots, sync_dirty, sync_appearance).chain(),
    );
    app.update();
    let entity = |id| app.world().non_send::<GuiRuntime>().entity_of(id).unwrap();
    let views = [entity(graph), entity(image), entity(text)];
    let font_view = entity(font);
    assert_eq!(
        app.world().get::<TextFont>(font_view).unwrap().font_size,
        bevy::text::FontSize::Px((original_size + 14) as f32)
    );
    for page in [1, 0] {
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_controller_page_index(root, "c1", page);
            gui.store_mut().advance_gear_tweens(10.0);
        }
        app.update();
    }
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(graph),
        Some(views[0])
    );
    let node = app.world().get::<Node>(views[0]).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (
            Val::Px(210.0),
            Val::Px(290.0),
            Val::Px(130.0),
            Val::Px(70.0)
        )
    );
    assert_eq!(
        app.world().get::<ImageNode>(views[1]).unwrap().color,
        backend::to_bevy_color(blue)
    );
    assert_eq!(
        app.world().get::<TextColor>(views[2]).unwrap().0,
        backend::to_bevy_color(orange)
    );
    assert_eq!(
        app.world().get::<TextFont>(font_view).unwrap().font_size,
        bevy::text::FontSize::Px(original_size as f32)
    );
    assert_eq!(
        *app.world()
            .get::<bevy::text::LineHeight>(font_view)
            .unwrap(),
        original_line_height
    );
}

#[test]
fn power_up_url_font_keeps_texture_colors_and_rebuilds_using_its_own_package() {
    let menu = FuiPackage::parse("MainMenu", MAIN_MENU).unwrap();
    let transition = FuiPackage::parse(
        "Transition",
        include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<crate::GuiFont>();
    let menu_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: menu.clone(),
            res_key: "menu/MainMenu".into(),
        });
    let transition_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: transition.clone(),
            res_key: "effects/Transition".into(),
        });
    let mut gui = GuiRuntime::default();
    gui.create_root(menu_handle, &menu, "Main").unwrap();
    let sample = gui
        .create_root(transition_handle, &transition, "PowerUp")
        .unwrap();
    let root = gui.root_object(sample).unwrap();
    let value = gui.store().child_by_name(root, "value").unwrap();
    gui.store_mut().set_object_text(value, "12345");
    app.insert_non_send(gui).add_systems(
        Update,
        (spawn_pending_roots, sync_dirty, sync_appearance).chain(),
    );
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(value)
        .unwrap();
    assert!(
        app.world().get::<Text>(entity).is_none(),
        "exported font URL must use glyphs"
    );
    let old = app.world().get::<BitmapGlyphs>(entity).unwrap().0.clone();
    assert_eq!(old.len(), 5);
    assert_eq!(
        app.world().get::<ImageNode>(old[0]).unwrap().color,
        Color::srgb(1.0, 1.0, 1.0)
    );
    let old_rect = app.world().get::<ImageNode>(old[2]).unwrap().rect;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(value, "12000");
    app.update();
    let new = &app.world().get::<BitmapGlyphs>(entity).unwrap().0;
    assert_eq!(new.len(), 5);
    assert!(app.world().get_entity(old[0]).is_err());
    assert_ne!(app.world().get::<ImageNode>(new[2]).unwrap().rect, old_rect);
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(app.world().get::<ImageNode>(new[2]).unwrap().image.id())
        .unwrap();
    assert!(
        path.path()
            .to_string_lossy()
            .starts_with("effects/Transition")
    );
}
