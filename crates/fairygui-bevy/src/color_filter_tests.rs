use super::*;
use fairygui_core::color_matrix::{ColorFilterParams, ColorMatrix};
use fairygui_core::{color::Color as GuiColor, package::ObjectType};

fn material(app: &App, view: Entity) -> &FillMaterial {
    let handle = &app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0;
    app.world()
        .resource::<Assets<FillMaterial>>()
        .get(handle)
        .unwrap()
}

fn assert_matrix(app: &App, view: Entity, expected: ColorMatrix) {
    let u = &material(app, view).uniform;
    assert_eq!(u.effects.x, 1);
    for r in 0..4 {
        assert_eq!(
            u.color_rows[r],
            Vec4::from_slice(&expected.values[r * 5..r * 5 + 4])
        );
    }
    assert_eq!(
        u.color_offset,
        Vec4::new(
            expected.values[4],
            expected.values[9],
            expected.values[14],
            expected.values[19]
        )
    );
}

#[test]
fn local_image_filter_updates_without_double_tint_and_gray_restores_changed_filter() {
    let (mut app, id, view) = super::tests::fixture(ObjectType::Image);
    let filter = ColorFilterParams {
        brightness: 0.2,
        contrast: -0.3,
        saturation: 0.6,
        hue: 0.4,
    };
    let raw = Color::srgba(0.7, 0.2, 0.4, 0.6).to_linear();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(id, Some(parent));
        gui.store_mut().set_alpha(parent, 0.5);
        gui.store_mut().set_alpha(id, 0.5);
        gui.store_mut()
            .set_control_color(id, GuiColor::new(0.7, 0.2, 0.4, 0.6), None);
        gui.store_mut().set_image_fill(id, 0, 0, false, 1.0);
        gui.store_mut()
            .set_color_filter(id, Some(ColorFilterParams::default()));
    }
    app.update();
    assert!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .is_none(),
        "zero defaults need no material"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(id, Some(filter));
    for _ in 0..8 {
        app.update();
    }
    let handle = app
        .world()
        .get::<MaterialNode<FillMaterial>>(view)
        .unwrap()
        .0
        .clone();
    assert_matrix(&app, view, filter.to_matrix());
    assert_eq!(
        material(&app, view).uniform.tint,
        Vec4::new(raw.red, raw.green, raw.blue, 0.15)
    );
    assert_eq!(
        app.world().get::<ImageNode>(view).unwrap().color,
        Color::NONE
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(id, true);
    app.update();
    assert_matrix(&app, view, ColorMatrix::LAYA_GRAY);
    let changed = ColorFilterParams {
        saturation: -1.0,
        ..Default::default()
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(id, Some(changed));
    app.update();
    assert_matrix(&app, view, ColorMatrix::LAYA_GRAY);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(id, false);
    app.update();
    assert_matrix(&app, view, changed.to_matrix());
    assert_eq!(
        app.world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0,
        handle
    );
    // Disabling the filter retains a still-active fill and its material handle.
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_image_fill(id, 3, 0, true, 0.5);
        gui.store_mut().set_color_filter(id, None);
    }
    app.update();
    assert_eq!(material(&app, view).uniform.effects.x, 0);
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
            .is_none()
    );
    assert_eq!(
        app.world().get::<ImageNode>(view).unwrap().color,
        Color::srgba(0.7, 0.2, 0.4, 0.15)
    );
    drop(handle);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Assets<FillMaterial>>().len(), 0);
}

#[test]
fn movie_and_loader_trimmed_visuals_share_local_matrix_and_keep_current_frame_texture() {
    for kind in [ObjectType::MovieClip, ObjectType::Loader] {
        let (mut app, id, root) = super::tests::fixture(kind);
        let image = app
            .world_mut()
            .entity_mut(root)
            .take::<ImageNode>()
            .unwrap();
        let view = app
            .world_mut()
            .spawn((
                ContentVisual {
                    root,
                    source_size: Vec2::new(100.0, 50.0),
                    trim: None,
                },
                Node {
                    left: Val::Px(20.0),
                    top: Val::Px(10.0),
                    width: Val::Px(120.0),
                    height: Val::Px(60.0),
                    ..default()
                },
                image,
            ))
            .id();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_image_fill(id, 0, 0, false, 1.0);
            gui.store_mut().set_color_filter(
                id,
                Some(ColorFilterParams {
                    hue: 0.5,
                    ..Default::default()
                }),
            );
        }
        app.update();
        assert_matrix(
            &app,
            view,
            ColorFilterParams {
                hue: 0.5,
                ..Default::default()
            }
            .to_matrix(),
        );
        let handle = app
            .world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0
            .clone();
        let texture = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        {
            let mut image = app.world_mut().get_mut::<ImageNode>(view).unwrap();
            image.image = texture.clone();
            image.rect = None;
        }
        app.update();
        assert_eq!(material(&app, view).texture, texture);
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
            .set_color_filter(id, None);
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
            Color::WHITE.to_linear()
        );
    }
}

#[test]
fn transition_filter_interpolation_reaches_the_image_material_with_zero_based_units() {
    use fairygui_core::transition::TransitionValue;
    let (mut app, id, view) = super::tests::fixture(ObjectType::Image);
    let start = TransitionValue::ColorFilter([0.0; 4]);
    let end = TransitionValue::ColorFilter([0.2, -0.4, -1.0, 0.6]);
    let mut handle = None;
    for t in [0.25, 0.5, 1.0] {
        let TransitionValue::ColorFilter(p) = start.lerp(&end, t) else {
            panic!("filter value")
        };
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_color_filter(
                id,
                Some(ColorFilterParams {
                    brightness: p[0],
                    contrast: p[1],
                    saturation: p[2],
                    hue: p[3],
                }),
            );
        app.update();
        let expected = ColorFilterParams {
            brightness: 0.2 * t,
            contrast: -0.4 * t,
            saturation: -t,
            hue: 0.6 * t,
        };
        assert_matrix(&app, view, expected.to_matrix());
        let current = app
            .world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0
            .clone();
        if let Some(previous) = &handle {
            assert_eq!(&current, previous);
        }
        handle = Some(current);
    }
}

#[test]
fn filtering_and_grayed_toggle_preserve_native_grid_uvs_mask_flip_and_texture() {
    let (mut app, id, view) = super::tests::fixture(ObjectType::Image);
    app.world_mut().entity_mut(view).insert(ImageGrid {
        source: Vec2::new(64.0, 32.0),
        offset: Vec2::ZERO,
        grid: fairygui_core::geom::Rect::new(8.0, 4.0, 40.0, 20.0),
        tiled: true,
    });
    app.world_mut().get_mut::<ImageNode>(view).unwrap().flip_x = true;
    app.update();
    let before = material(&app, view).clone();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(id, true);
    app.update();
    assert_matrix(&app, view, ColorMatrix::LAYA_GRAY);
    let after = material(&app, view);
    assert_eq!(before.texture, after.texture);
    assert_eq!(before.uniform.flags, after.uniform.flags);
    assert_eq!(before.uniform.points, after.uniform.points);
    assert_eq!(before.uniform.grid_u, after.uniform.grid_u);
    assert_eq!(before.uniform.repeat, after.uniform.repeat);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(id, false);
    app.update();
    assert_eq!(material(&app, view).uniform, before.uniform);
}

#[test]
fn serialized_component_gray_reaches_leaf_materials_but_controller_skins_keep_their_colors() {
    use crate::runtime::{spawn_pending_roots, sync_appearance, sync_dirty};
    use bevy::render::render_resource::Extent3d;
    let package = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Grayed.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<crate::FuiAsset>();
    install(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<crate::FuiAsset>>()
        .add(crate::FuiAsset {
            package: package.clone(),
            res_key: "gray/Basics".into(),
        });
    let mut atlas_handles = Vec::new();
    for item in package
        .items()
        .iter()
        .filter(|i| i.item_type == crate::core::package::PackageItemType::Atlas)
    {
        let handle: Handle<Image> = app
            .world()
            .resource::<AssetServer>()
            .load(format!("gray/{}", item.file.as_ref().unwrap()));
        let mut texture = Image::default();
        texture.resize(Extent3d {
            width: 2048,
            height: 2048,
            depth_or_array_layers: 1,
        });
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(handle.id(), texture)
            .unwrap();
        atlas_handles.push(handle);
    }
    let mut gui = GuiRuntime::default();
    let index = gui
        .create_root(handle.clone(), &package, "Demo_Component")
        .unwrap();
    let root = gui.root_object(index).unwrap();
    let nested = gui.store().child_by_name(root, "n17").unwrap();
    let image = gui.store().child_by_name(nested, "n1").unwrap();
    let text = gui.store().child_by_name(nested, "n2").unwrap();
    let index = gui.create_root(handle, &package, "Demo_Button").unwrap();
    let buttons = gui.root_object(index).unwrap();
    let controlled = gui.store().child_by_name(buttons, "n35").unwrap();
    let title = gui.store().child_by_name(controlled, "title").unwrap();
    let disabled = gui.store().child_by_name(buttons, "n38").unwrap();
    let skin = gui.store().child_by_name(disabled, "n4").unwrap();
    let title_color = gui
        .store()
        .object(title)
        .unwrap()
        .control()
        .as_text_field()
        .unwrap()
        .color;
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            sync_dirty,
            sync_appearance,
            sync_image_fills,
        )
            .chain(),
    );
    app.update();
    let view = |id| app.world().non_send::<GuiRuntime>().entity_of(id).unwrap();
    let (image_view, text_view, title_view, skin_view) =
        (view(image), view(text), view(title), view(skin));
    let texture = &app
        .world()
        .get::<ImageNode>(image_view)
        .expect("exported image descriptor")
        .image;
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .contains(texture.id()),
        "test atlas remains loaded: {texture:?}"
    );
    assert_matrix(&app, image_view, ColorMatrix::LAYA_GRAY);
    let gray = app
        .world()
        .get::<TextColor>(text_view)
        .unwrap()
        .0
        .to_srgba();
    assert!((gray.red - 2.0 / 3.0).abs() < 1e-6 && (gray.green - gray.red).abs() < 1e-6);
    assert_eq!(
        app.world().get::<TextColor>(title_view).unwrap().0,
        backend::to_bevy_color(title_color)
    );
    if let Some(handle) = app.world().get::<MaterialNode<FillMaterial>>(skin_view) {
        assert_eq!(
            app.world()
                .resource::<Assets<FillMaterial>>()
                .get(&handle.0)
                .unwrap()
                .uniform
                .effects
                .x,
            0,
            "controller skin has no extra gray matrix"
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_grayed(nested, false);
    app.update();
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(image),
        Some(image_view)
    );
    if let Some(handle) = app.world().get::<MaterialNode<FillMaterial>>(image_view) {
        assert_eq!(
            app.world()
                .resource::<Assets<FillMaterial>>()
                .get(&handle.0)
                .unwrap()
                .uniform
                .effects
                .x,
            0
        );
    }
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(image)
            .unwrap()
            .grayed()
    );
}
