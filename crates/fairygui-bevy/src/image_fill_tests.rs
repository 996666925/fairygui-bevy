use super::*;
use bevy::render::render_resource::Extent3d;
use fairygui_core::{color::Color as GuiColor, package::ObjectType};

#[test]
fn offscreen_filters_keep_encoded_texture_tint_without_applying_the_filter_twice() {
    let (mut app, id, view) = fixture(ObjectType::Image);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.compositing = true;
        gui.store_mut().set_image_fill(id, 0, 0, false, 1.);
        gui.store_mut()
            .set_control_color(id, GuiColor::from_rgba8(50, 100, 150, 255), None);
        gui.store_mut().set_color_filter(
            id,
            Some(fairygui_core::color_matrix::ColorFilterParams {
                brightness: 0.2,
                ..default()
            }),
        );
    }
    app.update();
    let handle = &app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0;
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(handle)
        .unwrap();
    assert_eq!(
        material.uniform.color_rows,
        [Vec4::X, Vec4::Y, Vec4::Z, Vec4::W]
    );
    assert_eq!(material.uniform.color_offset, Vec4::ZERO);
    assert_eq!(material.uniform.effects.x, 1);
    assert_eq!(
        app.world().get::<ImageNode>(view).unwrap().color,
        Color::NONE
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(id, None);
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_none()
    );
    assert!(
        (app.world()
            .get::<ImageNode>(view)
            .unwrap()
            .color
            .to_srgba()
            .red
            - 50. / 255.)
            .abs()
            < 1e-6
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(id, Some(parent));
        gui.store_mut().set_color_filter(
            parent,
            Some(fairygui_core::color_matrix::ColorFilterParams::default()),
        );
    }
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_some()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_control_color(id, GuiColor::WHITE, None);
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_none()
    );
}

pub(super) fn fixture(kind: ObjectType) -> (App, fairygui_core::GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>();
    install(&mut app);
    let mut texture = Image::default();
    texture.resize(Extent3d {
        width: 256,
        height: 128,
        depth_or_array_layers: 1,
    });
    let texture = app.world_mut().resource_mut::<Assets<Image>>().add(texture);
    let mut gui = GuiRuntime::default();
    let id = gui.store_mut().create(kind);
    gui.store_mut().set_size(id, 200.0, 100.0);
    gui.store_mut().set_image_fill(id, 5, 0, true, 0.25);
    let view = app
        .world_mut()
        .spawn((
            GObjectRef(id),
            Node {
                left: Val::Px(30.0),
                top: Val::Px(60.0),
                width: Val::Px(200.0),
                height: Val::Px(100.0),
                ..default()
            },
            ImageNode {
                image: texture,
                rect: Some(Rect::new(32.0, 16.0, 96.0, 48.0)),
                ..default()
            },
        ))
        .id();
    app.insert_non_send(gui).add_systems(
        Update,
        (crate::runtime::sync_appearance, sync_image_fills).chain(),
    );
    (app, id, view)
}

#[test]
fn fill_keeps_layout_and_atlas_descriptor_stable_and_restores_normal_rendering_on_disable() {
    let (mut app, id, view) = fixture(ObjectType::Image);
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0
        .clone();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.flags.x, 4);
    assert_eq!(
        material.uniform.points[..4],
        [
            Vec4::new(100.0, 50.0, 0.0, 0.0),
            Vec4::new(200.0, 50.0, 0.0, 0.0),
            Vec4::new(200.0, 0.0, 0.0, 0.0),
            Vec4::new(100.0, 0.0, 0.0, 0.0)
        ]
    );
    assert_eq!(
        material.uniform.atlas,
        Vec4::new(0.125, 0.125, 0.375, 0.375)
    );
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(node.width, Val::Px(200.0));
    assert_eq!(node.left, Val::Px(30.0));
    let image = app.world().get::<ImageNode>(view).unwrap();
    assert_eq!(image.rect, Some(Rect::new(32.0, 16.0, 96.0, 48.0)));
    assert_eq!(image.color, Color::NONE);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_image_fill_amount(id, 0.0);
        gui.store_mut().set_alpha(id, 0.5);
        gui.store_mut()
            .set_control_color(id, GuiColor::new(1.0, 0.0, 0.0, 1.0), None);
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0,
        handle
    );
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.flags.x, 0);
    assert_eq!(material.uniform.tint, Vec4::new(1.0, 0.0, 0.0, 0.5));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill(id, 0, 0, false, 1.0);
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_none()
    );
    assert_eq!(
        app.world()
            .get::<ImageNode>(view)
            .unwrap()
            .color
            .to_linear(),
        Color::srgba(1.0, 0.0, 0.0, 0.5).to_linear()
    );
    drop(handle);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<FillMaterial>>().len(), 0);
}

#[test]
fn loader_mask_uses_full_aligned_canvas_before_trim_and_retains_the_material_on_texture_changes() {
    let (mut app, id, root) = fixture(ObjectType::Loader);
    let image = app
        .world_mut()
        .entity_mut(root)
        .take::<ImageNode>()
        .unwrap();
    let visual = app
        .world_mut()
        .spawn((
            ContentVisual {
                root,
                source_size: Vec2::new(100.0, 100.0),
                trim: Some((Vec2::new(10.0, 20.0), Vec2::new(64.0, 32.0))),
            },
            Node {
                left: Val::Px(60.0),
                top: Val::Px(20.0),
                width: Val::Px(64.0),
                height: Val::Px(32.0),
                ..default()
            },
            image,
        ))
        .id();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_loader_layout(id, 0, 1, 1);
    }
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(visual)
        .unwrap()
        .0
        .clone();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.domain, Vec4::new(10.0, 20.0, 64.0, 32.0));
    assert_eq!(material.uniform.points[0], Vec4::new(50.0, 50.0, 0.0, 0.0));
    let texture = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    app.world_mut().get_mut::<ImageNode>(visual).unwrap().image = texture.clone();
    app.update();
    assert_eq!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(visual)
            .unwrap()
            .0,
        handle
    );
    assert_eq!(
        app.world()
            .resource::<Assets<FillMaterial>>()
            .get(&handle)
            .unwrap()
            .texture,
        texture
    );
}

#[test]
fn sliced_tiled_and_flipped_images_keep_uv_parameters_while_the_mask_changes() {
    let (mut app, id, view) = fixture(ObjectType::Image);
    {
        let mut image = app.world_mut().get_mut::<ImageNode>(view).unwrap();
        image.flip_x = true;
        image.image_mode = NodeImageMode::Sliced(bevy::sprite::TextureSlicer {
            border: bevy::sprite::BorderRect::from([8.0, 8.0, 4.0, 4.0]),
            ..default()
        });
    }
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0
        .clone();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(
        material.uniform.source_slices,
        Vec4::new(0.125, 0.125, 0.875, 0.875)
    );
    assert_eq!(
        material.uniform.target_slices,
        Vec4::new(0.04, 0.04, 0.96, 0.96)
    );
    assert_eq!(material.uniform.flags.y, 1);
    app.world_mut()
        .get_mut::<ImageNode>(view)
        .unwrap()
        .image_mode = NodeImageMode::Tiled {
        tile_x: true,
        tile_y: false,
        stretch_value: 1.0,
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill(id, 3, 3, false, 0.5);
    app.update();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.repeat, Vec4::new(1.0, 1.0, 3.125, 1.0));
    assert_eq!(material.uniform.flags.y, 1);
}

#[test]
fn native_grid_renders_without_fill_and_keeps_its_handle_when_masking_changes() {
    let (mut app, id, view) = fixture(ObjectType::Image);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill(id, 0, 0, false, 1.0);
    app.world_mut().entity_mut(view).insert(ImageGrid {
        source: Vec2::new(64.0, 32.0),
        offset: Vec2::ZERO,
        grid: fairygui_core::geom::Rect::new(8.0, 4.0, 40.0, 20.0),
        tiled: true,
    });
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0
        .clone();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.flags.w, 1);
    assert_eq!(material.uniform.flags.x, 4);
    assert_eq!(
        material.uniform.target_slices,
        Vec4::new(0.04, 0.04, 0.92, 0.92)
    );
    assert_eq!(
        material.uniform.grid_u,
        Vec4::new(0.125, 0.15625, 0.3125, 0.375)
    );
    assert!((material.uniform.repeat.x - 4.4).abs() < 0.00001);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill(id, 5, 0, true, 0.25);
    app.update();
    assert_eq!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0,
        handle
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_image_fill(id, 0, 0, false, 1.0);
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_some()
    );
    app.world_mut().entity_mut(view).remove::<ImageGrid>();
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_none()
    );
}

#[test]
fn native_grid_compresses_only_the_small_axis_and_expands_trimmed_uvs() {
    let (mut app, id, view) = fixture(ObjectType::Image);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(id, 16.0, 100.0);
    app.world_mut().get_mut::<Node>(view).unwrap().width = Val::Px(16.0);
    app.world_mut().entity_mut(view).insert(ImageGrid {
        source: Vec2::new(64.0, 32.0),
        offset: Vec2::new(4.0, 2.0),
        grid: fairygui_core::geom::Rect::new(8.0, 4.0, 40.0, 20.0),
        tiled: true,
    });
    app.update();
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0
        .clone();
    let material = app
        .world()
        .resource::<Assets<FillMaterial>>()
        .get(&handle)
        .unwrap();
    assert!((material.uniform.target_slices.x - 1.0 / 3.0).abs() < 0.00001);
    assert_eq!(
        material.uniform.target_slices.x,
        material.uniform.target_slices.z
    );
    assert_eq!(material.uniform.target_slices.y, 0.04);
    assert_eq!(material.uniform.target_slices.w, 0.92);
    assert_eq!(material.uniform.grid_u.x, 28.0 / 256.0);
    assert_eq!(material.uniform.clip.x, 32.0 / 256.0);
}

#[test]
fn exported_foreign_loader_grid_has_full_canvas_layout_and_cleans_up_on_url_change() {
    use crate::runtime::{
        content::sync_content_layout, spawn_pending_objects, spawn_pending_roots, sync_dirty,
        sync_movie_sources, sync_stage_hierarchy,
    };
    let basics = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let nine = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/NineGrid.fui"),
    )
    .unwrap();
    let item = nine
        .items()
        .iter()
        .enumerate()
        .find(|(_, item)| item.scale9_grid.is_some())
        .unwrap();
    let resource = format!("ui://{}{}", nine.id(), item.1.id);
    let atlas = nine
        .item(nine.sprite_for(item.0).unwrap().atlas_item)
        .unwrap()
        .file
        .clone()
        .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<crate::FuiAsset>()
        .init_resource::<crate::GuiFont>();
    install(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<crate::FuiAsset>>()
        .add(crate::FuiAsset {
            package: basics.clone(),
            res_key: "wrong/Basics".into(),
        });
    let _nine = app
        .world_mut()
        .resource_mut::<Assets<crate::FuiAsset>>()
        .add(crate::FuiAsset {
            package: nine,
            res_key: "grid/Basics".into(),
        });
    let image: Handle<Image> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("grid/{atlas}"));
    let mut texture = Image::default();
    texture.resize(Extent3d {
        width: 2048,
        height: 2048,
        depth_or_array_layers: 1,
    });
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(image.id(), texture)
        .unwrap();
    let mut gui = GuiRuntime::default();
    let root = gui.create_root(handle, &basics, "Demo_Loader").unwrap();
    let loader = gui
        .store()
        .child_by_name(gui.root_object(root).unwrap(), "n2")
        .unwrap();
    gui.store_mut().apply_icon_to(loader, &resource);
    gui.store_mut().set_size(loader, 233.0, 117.0);
    gui.store_mut().set_loader_layout(loader, 4, 0, 0);
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_movie_sources,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_content_layout,
            sync_dirty,
            crate::runtime::sync_appearance,
            sync_image_fills,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();
    let root_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(loader)
        .unwrap();
    let visual = app
        .world()
        .get::<crate::runtime::MovieSource>(root_view)
        .unwrap()
        .visual
        .unwrap();
    assert!(app.world().get::<ImageGrid>(visual).unwrap().tiled);
    let node = app.world().get::<Node>(visual).unwrap();
    assert_eq!(node.width, Val::Px(233.0));
    assert_eq!(node.height, Val::Px(117.0));
    let material = &app
        .world()
        .get::<MaterialNode<FillMaterial>>(visual)
        .unwrap()
        .0;
    assert_eq!(
        app.world()
            .resource::<Assets<FillMaterial>>()
            .get(material)
            .unwrap()
            .texture,
        image
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .apply_icon_to(loader, "ui://Basics/VtSeperator");
    app.update();
    assert!(app.world().get_entity(visual).is_err());
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(loader),
        Some(root_view)
    );
    let replacement = app
        .world()
        .get::<crate::runtime::MovieSource>(root_view)
        .unwrap()
        .visual
        .unwrap();
    assert!(app.world().get::<ImageGrid>(replacement).is_none());
}
