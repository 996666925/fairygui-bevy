use super::*;
use crate::runtime::appearance::TextTintSource;
use fairygui_core::{
    color::Color as GuiColor,
    color_matrix::{ColorFilterParams, ColorMatrix},
    package::ObjectType,
};

fn parse_color(value: &str) -> GuiColor {
    let v = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap();
    GuiColor::from_rgba8((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}
fn uniform(app: &App, view: Entity) -> Option<&FillUniform> {
    let handle = &app.world().get::<MaterialNode<FillMaterial>>(view)?.0;
    Some(
        &app.world()
            .resource::<Assets<FillMaterial>>()
            .get(handle)?
            .uniform,
    )
}

fn exported_fixture(bitmap: bool) -> (App, usize, fairygui_core::GObjectId) {
    use crate::{asset::FuiAsset, runtime::*};
    let bytes: &[u8] = if bitmap {
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui")
    } else {
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui")
    };
    let package = fairygui_core::FuiPackage::parse("Basics", bytes).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>();
    install(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Demo_Text").unwrap();
    let root = gui.root_object(index).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            sync_dirty,
            sync_rich_text,
            sync_text_sizes,
            sync_appearance,
            sync_stage_hierarchy,
            sync_inline_image_hierarchy,
            fixture_textures,
            sync_image_fills,
        )
            .chain(),
    );
    app.update();
    app.update();
    (app, index, root)
}

// The headless package fixture has no async renderer/image loader. Give its
// real atlas handles storage so material binding can be exercised; GPU smoke
// loads the actual package PNG instead.
fn fixture_textures(views: Query<&ImageNode>, mut images: ResMut<Assets<Image>>) {
    for image in &views {
        if images.get(&image.image).is_none() {
            let mut texture = Image::default();
            texture.resize(bevy::render::render_resource::Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: 1,
            });
            images.insert(image.image.id(), texture).unwrap();
        }
    }
}

fn sampled_parts(app: &App, owner: fairygui_core::GObjectId) -> Vec<Entity> {
    use crate::runtime::{BitmapGlyphs, InlineImageParts};
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    if let Some(parts) = app.world().get::<BitmapGlyphs>(view) {
        parts.0.clone()
    } else {
        app.world()
            .get::<InlineImageParts>(view)
            .unwrap()
            .iter()
            .collect()
    }
}

#[test]
fn exported_bitmap_fonts_and_inline_images_keep_sampling_geometry_through_effects_and_rebuilds() {
    for (bitmap, name, tinted) in [
        (true, "n0", false),
        (true, "n2", true),
        (false, "n12", false),
    ] {
        let (mut app, index, root) = exported_fixture(bitmap);
        let owner = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .child_by_name(root, name)
            .unwrap();
        let parts = sampled_parts(&app, owner);
        assert!(!parts.is_empty());
        let geometry: Vec<_> = parts
            .iter()
            .map(|&e| {
                let image = app.world().get::<ImageNode>(e).unwrap();
                (
                    image.image.clone(),
                    image.rect,
                    app.world().get::<Node>(e).unwrap().clone(),
                )
            })
            .collect();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_alpha(root, 0.5);
            gui.store_mut().set_alpha(owner, 0.6);
            gui.store_mut().set_color_filter(
                owner,
                Some(ColorFilterParams {
                    hue: 0.5,
                    ..default()
                }),
            );
        }
        app.update();
        let handles: Vec<_> = parts
            .iter()
            .map(|&e| {
                app.world()
                    .get::<MaterialNode<FillMaterial>>(e)
                    .unwrap()
                    .0
                    .clone()
            })
            .collect();
        for gray in [true, false, true] {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_grayed(owner, gray);
            app.update();
            assert_eq!(sampled_parts(&app, owner), parts);
            for (i, &part) in parts.iter().enumerate() {
                assert_eq!(
                    app.world()
                        .get::<MaterialNode<FillMaterial>>(part)
                        .unwrap()
                        .0,
                    handles[i]
                );
                let u = uniform(&app, part).unwrap();
                assert!((u.tint.w - 0.3).abs() < 1e-6);
                if gray {
                    let expected = if tinted {
                        Color::srgb(2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0).to_linear().red
                    } else {
                        1.0
                    };
                    assert!((u.tint.x - expected).abs() < 1e-6);
                }
                let image = app.world().get::<ImageNode>(part).unwrap();
                assert_eq!((&image.image, image.rect), (&geometry[i].0, geometry[i].1));
                let node = app.world().get::<Node>(part).unwrap();
                assert_eq!(
                    (node.left, node.top, node.width, node.height),
                    (
                        geometry[i].2.left,
                        geometry[i].2.top,
                        geometry[i].2.width,
                        geometry[i].2.height
                    )
                );
            }
        }
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_grayed(owner, false);
            gui.store_mut().set_color_filter(owner, None);
        }
        app.update();
        assert!(parts.iter().all(|&e| uniform(&app, e).is_none()));
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_color_filter(
                owner,
                Some(ColorFilterParams {
                    brightness: 0.2,
                    ..default()
                }),
            );
        app.update();
        // Rebuild the actual package source while filtered, including linked sibling disposal.
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(
                owner,
                if bitmap {
                    "123UI"
                } else {
                    "[img]ui://9leh0eyfrpmb6[/img][img]ui://9leh0eyfrpmb6[/img]"
                },
            );
        app.update();
        app.update();
        assert!(parts.iter().all(|&e| app.world().get_entity(e).is_err()));
        let next = sampled_parts(&app, owner);
        assert!(!next.is_empty());
        assert!(next.iter().all(|&e| uniform(&app, e).is_some()));
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
        gui.remove_root(&mut Commands::new(&mut queue, app.world()), index);
        queue.apply(app.world_mut());
        app.insert_non_send(gui);
        drop(handles);
        for _ in 0..3 {
            app.update();
        }
        assert!(next.iter().all(|&e| app.world().get_entity(e).is_err()));
        assert_eq!(app.world().resource::<Assets<FillMaterial>>().len(), 0);
    }
}

#[test]
fn native_text_draw_tints_and_legacy_matrices_match_sampled_text_image_materials() {
    let mut state = None;
    let mut cases = 0;
    let mut steps = 0;
    let mut samples = 0;
    let mut current = ColorMatrix::IDENTITY;
    let mut draw_color = GuiColor::WHITE;
    for line in include_str!("../tests/fixtures/text-image-color-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "case" => {
                let (mut app, owner, view) = super::tests::fixture(ObjectType::Text);
                let override_color = if p[3] != "-" {
                    Some(parse_color(p[3]))
                } else if p[2] == "0" {
                    Some(GuiColor::WHITE)
                } else {
                    None
                };
                app.world_mut()
                    .entity_mut(view)
                    .remove::<GObjectRef>()
                    .insert(TextTintSource {
                        owner,
                        color: override_color,
                    });
                {
                    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                    gui.store_mut().set_size(owner, 1.0, 1.0);
                    gui.store_mut()
                        .set_control_color(owner, parse_color("#326496"), None);
                }
                state = Some((app, owner, view));
                cases += 1;
            }
            "step" => {
                let (app, owner, view) = state.as_mut().unwrap();
                draw_color = parse_color(p[2]);
                {
                    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                    let store = gui.store_mut();
                    match p[1] {
                        "color" => {}
                        "filter" => store.set_color_filter(
                            *owner,
                            Some(ColorFilterParams {
                                brightness: 0.12,
                                contrast: -0.3,
                                saturation: 0.7,
                                hue: 0.4,
                            }),
                        ),
                        "gray" => store.set_grayed(*owner, true),
                        "changed_gray" => store.set_color_filter(
                            *owner,
                            Some(ColorFilterParams {
                                brightness: -0.2,
                                contrast: 0.4,
                                saturation: -0.5,
                                hue: -0.3,
                            }),
                        ),
                        "restored" => store.set_grayed(*owner, false),
                        "new_color" => {
                            store.set_control_color(*owner, parse_color("#D04488"), None)
                        }
                        "disabled_filter" => store.set_color_filter(*owner, None),
                        _ => panic!("{line}"),
                    }
                }
                app.update();
                steps += 1;
                let tint = if let Some(u) = uniform(app, *view) {
                    assert_eq!(u.domain, Vec4::new(0.0, 0.0, 200.0, 100.0));
                    assert!(
                        u.points.iter().any(|p| p.x == 200.0 && p.y == 100.0),
                        "full sampled quad is independent of tiny text owner"
                    );
                    Color::linear_rgba(u.tint.x, u.tint.y, u.tint.z, u.tint.w).to_srgba()
                } else {
                    app.world()
                        .get::<ImageNode>(*view)
                        .unwrap()
                        .color
                        .to_srgba()
                };
                for (got, expected) in [tint.red, tint.green, tint.blue].into_iter().zip([
                    draw_color.r,
                    draw_color.g,
                    draw_color.b,
                ]) {
                    assert!((got - expected).abs() < 2e-6, "{line}: {got} != {expected}");
                }
            }
            "matrix" => {
                let values: Vec<f32> = p[1..].iter().map(|s| s.parse().unwrap()).collect();
                current = ColorMatrix {
                    values: values.try_into().unwrap(),
                };
                let (app, _, view) = state.as_ref().unwrap();
                let actual = uniform(app, *view)
                    .map(|u| ColorMatrix {
                        values: std::array::from_fn(|i| {
                            if i % 5 == 4 {
                                u.color_offset[i / 5]
                            } else {
                                u.color_rows[i / 5][i % 5]
                            }
                        }),
                    })
                    .unwrap_or_default();
                for (got, want) in actual.values.into_iter().zip(current.values) {
                    assert!((got - want).abs() < 2e-6);
                }
            }
            "sample" => {
                let n: Vec<f32> = p[1..].iter().map(|s| s.parse().unwrap()).collect();
                let input = GuiColor::new(
                    n[0] * draw_color.r,
                    n[1] * draw_color.g,
                    n[2] * draw_color.b,
                    n[3],
                );
                let color = current.apply(input);
                for (got, want) in [color.r, color.g, color.b, color.a]
                    .into_iter()
                    .zip(n[4..].iter().copied())
                {
                    assert!((got - want).abs() < 2e-6);
                }
                samples += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!((cases, steps, samples), (4, 28, 84));
}

#[test]
fn text_image_filter_keeps_handles_atlas_geometry_and_alpha_and_restores_the_raw_image() {
    for kind in [ObjectType::Text, ObjectType::InputText] {
        let (mut app, owner, view) = super::tests::fixture(kind);
        app.world_mut()
            .entity_mut(view)
            .remove::<GObjectRef>()
            .insert(TextTintSource {
                owner,
                color: Some(GuiColor::WHITE),
            });
        let descriptor = app.world().get::<ImageNode>(view).unwrap().clone();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let parent = gui.store_mut().create(ObjectType::Component);
            gui.store_mut().set_parent(owner, Some(parent));
            gui.store_mut().set_alpha(parent, 0.5);
            gui.store_mut().set_alpha(owner, 0.6);
            gui.store_mut().set_color_filter(
                owner,
                Some(ColorFilterParams {
                    hue: 0.5,
                    ..Default::default()
                }),
            );
        }
        app.update();
        let handle = app
            .world()
            .get::<MaterialNode<FillMaterial>>(view)
            .unwrap()
            .0
            .clone();
        assert!((uniform(&app, view).unwrap().tint.w - 0.3).abs() < 1e-6);
        for gray in [true, false, true, false] {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_grayed(owner, gray);
            app.update();
            assert_eq!(
                app.world()
                    .get::<MaterialNode<FillMaterial>>(view)
                    .unwrap()
                    .0,
                handle
            );
        }
        assert_eq!(
            app.world().get::<ImageNode>(view).unwrap().rect,
            descriptor.rect
        );
        assert_eq!(
            app.world().get::<ImageNode>(view).unwrap().image,
            descriptor.image
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_color_filter(owner, None);
        app.update();
        assert!(
            app.world()
                .get::<MaterialNode<FillMaterial>>(view)
                .is_none()
        );
        let raw = app.world().get::<ImageNode>(view).unwrap().color.to_srgba();
        assert_eq!((raw.red, raw.green, raw.blue), (1.0, 1.0, 1.0));
        assert!((raw.alpha - 0.3).abs() < 1e-6);
        drop(handle);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Assets<FillMaterial>>().len(), 0);
    }
}
