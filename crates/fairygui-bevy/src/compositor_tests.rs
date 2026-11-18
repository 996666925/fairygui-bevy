use super::*;
use fairygui_core::{color_matrix::ColorFilterParams, package::ObjectType};

fn tree() -> (GuiRuntime, [GObjectId; 3]) {
    let mut gui = GuiRuntime::default();
    gui.compositing = true;
    let a = gui.store_mut().create(ObjectType::Component);
    let b = gui.store_mut().create(ObjectType::Component);
    let c = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_parent(b, Some(a));
    gui.store_mut().set_parent(c, Some(b));
    for (id, alpha) in [(a, 0.5), (b, 0.4), (c, 0.6)] {
        gui.store_mut().set_alpha(id, alpha);
    }
    (gui, [a, b, c])
}

#[test]
fn writable_effects_update_isolation_and_blend_policies_without_replacing_views() {
    use crate::{FairyGuiPlugin, GuiBlendMode, GuiColorFilter};
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .add_systems(PostUpdate, sync_policies);
    let (gui, [a, b, c]) = tree();
    app.insert_non_send(gui);
    let parent = app.world_mut().spawn((Node::default(), GObjectRef(a))).id();
    let group = app.world_mut().spawn((Node::default(), GObjectRef(b))).id();
    let child = app.world_mut().spawn((Node::default(), GObjectRef(c))).id();
    app.update();
    app.world_mut().get_mut::<GuiBlendMode>(parent).unwrap().0 = Some(BlendMode::Add);
    app.world_mut().get_mut::<GuiColorFilter>(group).unwrap().0 = Some(default());
    app.update();
    let policy = app.world().get::<PaintPolicy>(child).unwrap();
    assert_eq!(policy.groups.len(), 1);
    assert_eq!(policy.groups[0].matrix, ColorMatrix::IDENTITY);
    assert_eq!(policy.groups[0].blend, BlendMode::Add);
    assert_eq!(policy.groups[0].opacity, 0.2);
    assert_eq!(policy.blend, BlendMode::Normal);
    assert_eq!(app.world().non_send::<GuiRuntime>().paint_alpha(c), 0.6);
    let tick = app
        .world()
        .entity(child)
        .get_ref::<PaintPolicy>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(child)
            .get_ref::<PaintPolicy>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut()
        .get_mut::<GuiColorFilter>(group)
        .unwrap()
        .0
        .as_mut()
        .unwrap()
        .hue = 0.5;
    app.world_mut().get_mut::<GuiBlendMode>(group).unwrap().0 = Some(BlendMode::Normal);
    app.update();
    let policy = app.world().get::<PaintPolicy>(child).unwrap();
    assert_eq!(policy.groups[0].blend, BlendMode::Normal);
    assert_eq!(
        policy.groups[0].matrix,
        ColorFilterParams {
            hue: 0.5,
            ..default()
        }
        .to_matrix()
    );
    app.world_mut().get_mut::<GuiColorFilter>(group).unwrap().0 = None;
    app.world_mut().get_mut::<GuiBlendMode>(group).unwrap().0 = None;
    app.update();
    let policy = app.world().get::<PaintPolicy>(child).unwrap();
    assert!(policy.groups.is_empty());
    assert_eq!(policy.blend, BlendMode::Add);
    assert!((app.world().non_send::<GuiRuntime>().paint_alpha(c) - 0.12).abs() < 1e-6);
}

#[test]
fn component_gray_does_not_add_an_unrequested_filter_to_its_bitmap_cache() {
    let (mut gui, [owner, _, child]) = tree();
    gui.store_mut().set_cache_as_bitmap(owner, true);
    gui.store_mut().set_grayed(owner, true);
    let object = gui.store().object(owner).unwrap();
    assert!(effect_matrix(object).is_none());
    assert!(gui.paint_matrix(object).is_none());
    assert_eq!(policy(&gui, child).groups[0].matrix, ColorMatrix::IDENTITY);
    assert!(gui.store().object(child).unwrap().grayed());
    let filter = ColorFilterParams {
        brightness: 0.15,
        ..default()
    };
    gui.store_mut().set_color_filter(owner, Some(filter));
    assert_eq!(policy(&gui, child).groups[0].matrix, filter.to_matrix());
    assert!(
        gui.paint_matrix(gui.store().object(owner).unwrap())
            .is_none()
    );
}

#[test]
fn normal_mask_separates_its_cache_from_content_and_reverse_keeps_paint_order() {
    let (mut gui, [a, b, mask]) = tree();
    let content = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_parent(content, Some(b));
    gui.store_mut().set_alpha(content, 0.8);
    gui.store_mut().set_blend_mode(a, BlendMode::Add);
    gui.store_mut().set_blend_mode(mask, BlendMode::Multiply);
    gui.store_mut().set_component_mask(b, Some(mask), false);
    let content_policy = policy(&gui, content);
    assert_eq!(content_policy.groups.len(), 1);
    assert_eq!(content_policy.groups[0].mask, Some(mask));
    assert_eq!(content_policy.groups[0].blend, BlendMode::Add);
    assert!((content_policy.groups[0].opacity - 0.2).abs() < 1e-6);
    assert_eq!(gui.paint_alpha(content), 0.8);
    let source = policy(&gui, mask);
    assert_eq!(source.mask_for, vec![b]);
    assert_eq!(source.groups.len(), 2);
    assert_eq!(
        (source.groups[1].blend, source.groups[1].opacity),
        (BlendMode::Normal, 0.6)
    );
    assert_eq!(gui.paint_alpha(mask), 1.);
    gui.store_mut().set_component_mask(b, Some(mask), true);
    let reversed = policy(&gui, mask);
    assert_eq!(reversed.groups.len(), 1);
    assert!(reversed.mask_for.is_empty());
    assert!(reversed.groups[0].mask.is_none());
    assert_eq!(reversed.blend, BlendMode::Erase);
    assert_eq!(gui.paint_alpha(mask), 0.6);
    gui.store_mut().set_component_mask(b, None, false);
    assert_eq!(policy(&gui, content).groups.len(), 1);
    gui.store_mut().set_cache_as_bitmap(b, false);
    assert!(policy(&gui, content).groups.is_empty());
}
#[test]
fn explicit_normal_overrides_inheritance_and_reset_restores_it() {
    let (mut gui, [a, b, c]) = tree();
    gui.store_mut().set_blend_mode(a, BlendMode::Add);
    assert_eq!(policy(&gui, c).blend, BlendMode::Add);
    gui.store_mut().set_blend_mode(b, BlendMode::Normal);
    assert!(gui.store().object(b).unwrap().blend_mode_is_set());
    assert_eq!(policy(&gui, c).blend, BlendMode::Normal);
    gui.store_mut().reset_blend_mode(b);
    assert!(!gui.store().object(b).unwrap().blend_mode_is_set());
    assert_eq!(policy(&gui, c).blend, BlendMode::Add);
    gui.store_mut().set_parent(c, None);
    assert_eq!(policy(&gui, c).blend, BlendMode::Normal);
}
#[test]
fn nested_isolation_transfers_alpha_and_blend_once_including_identity_filters() {
    let (mut gui, [a, b, c]) = tree();
    gui.store_mut().set_blend_mode(a, BlendMode::Add);
    gui.store_mut()
        .set_color_filter(b, Some(ColorFilterParams::default()));
    let outer = policy(&gui, c);
    assert_eq!(outer.groups.len(), 1);
    assert_eq!(
        (
            outer.groups[0].id,
            outer.groups[0].opacity,
            outer.groups[0].blend
        ),
        (b, 0.2, BlendMode::Add)
    );
    assert_eq!(outer.blend, BlendMode::Normal);
    assert_eq!(gui.paint_alpha(c), 0.6);
    gui.store_mut().set_blend_mode(c, BlendMode::Multiply);
    gui.store_mut().set_color_filter(
        c,
        Some(ColorFilterParams {
            brightness: 0.2,
            ..default()
        }),
    );
    let nested = policy(&gui, c);
    assert_eq!(nested.groups.len(), 2);
    assert_eq!(
        (nested.groups[1].opacity, nested.groups[1].blend),
        (0.6, BlendMode::Multiply)
    );
    assert_eq!(gui.paint_alpha(c), 1.);
    assert!(gui.paint_matrix(gui.store().object(c).unwrap()).is_none());
    gui.store_mut().set_color_filter(b, None);
    assert!((policy(&gui, c).groups[0].opacity - 0.12).abs() < 1e-6);
    gui.store_mut().set_color_filter(c, None);
    assert!(policy(&gui, c).groups.is_empty());
    assert!((gui.paint_alpha(c) - 0.12).abs() < 1e-6);
}
#[test]
fn headless_keeps_local_filter_and_effective_alpha_without_gpu() {
    let (mut gui, [_, _, c]) = tree();
    gui.compositing = false;
    gui.store_mut().set_color_filter(
        c,
        Some(ColorFilterParams {
            hue: 0.5,
            ..default()
        }),
    );
    assert!(gui.paint_matrix(gui.store().object(c).unwrap()).is_some());
    assert!((gui.paint_alpha(c) - 0.12).abs() < 1e-6);
}
#[test]
fn policy_ownership_follows_internal_text_inline_images_and_reparenting() {
    let (mut gui, [a, b, c]) = tree();
    gui.store_mut()
        .set_color_filter(b, Some(ColorFilterParams::default()));
    let mut app = App::new();
    app.insert_non_send(gui).add_systems(Update, sync_policies);
    let root = app.world_mut().spawn((Node::default(), GObjectRef(c))).id();
    let text = app
        .world_mut()
        .spawn((
            Node::default(),
            TextTintSource {
                owner: c,
                color: None,
            },
            ChildOf(root),
        ))
        .id();
    let glyph = app.world_mut().spawn((Node::default(), ChildOf(text))).id();
    let inline = app
        .world_mut()
        .spawn((Node::default(), InlineImageOwner(text)))
        .id();
    app.update();
    for entity in [root, text, glyph, inline] {
        assert_eq!(
            app.world().get::<PaintPolicy>(entity).unwrap().groups[0].id,
            b
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(c, Some(a));
    app.update();
    for entity in [root, text, glyph, inline] {
        assert!(
            app.world()
                .get::<PaintPolicy>(entity)
                .unwrap()
                .groups
                .is_empty()
        );
    }
    app.world_mut().entity_mut(root).despawn();
    app.update();
    assert!(app.world().get_entity(inline).is_err());
}

#[test]
fn matches_upstream_sprite_substructure_alpha_and_inherited_blend() {
    let input = include_str!("../../fairygui-core/tests/fixtures/blend-reference.txt");
    let mode = |s: &str| match s {
        "2" => BlendMode::Add,
        _ => BlendMode::Normal,
    };
    let mut state = None;
    let mut groups = 0;
    let mut cases = 0;
    for line in input.lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        match p.first().copied() {
            Some("case") => {
                let (mut gui, ids) = tree();
                let mask: usize = p[4].parse().unwrap();
                for i in 0..3 {
                    if p[i + 1] != "0" {
                        gui.store_mut().set_blend_mode(ids[i], mode(p[i + 1]));
                    }
                    if mask & (1 << i) != 0 {
                        gui.store_mut()
                            .set_color_filter(ids[i], Some(ColorFilterParams::default()));
                    }
                }
                state = Some((gui, ids));
                groups = 0;
                cases += 1;
            }
            Some("group") => {
                let (gui, ids) = state.as_ref().unwrap();
                let policy = policy(gui, ids[2]);
                let group = &policy.groups[groups];
                assert_eq!(group.id, ids[p[1].parse::<usize>().unwrap()]);
                assert_eq!(group.blend, mode(p[2]));
                assert!(
                    (group.opacity - p[3].parse::<f32>().unwrap()).abs() < 1e-6,
                    "{line}"
                );
                groups += 1;
            }
            Some("paint") => {
                let (gui, ids) = state.as_ref().unwrap();
                let policy = policy(gui, ids[2]);
                assert_eq!(policy.groups.len(), groups);
                assert_eq!(policy.blend, mode(p[1]));
                assert!(
                    (gui.paint_alpha(ids[2]) - p[2].parse::<f32>().unwrap()).abs() < 1e-6,
                    "{line}"
                );
            }
            _ => (),
        }
    }
    assert_eq!(cases, 216);
}

pub(super) fn clip_fixture(item: &str) -> (GuiRuntime, GObjectId, GObjectId) {
    let package = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let mut gui = GuiRuntime::default();
    gui.compositing = true;
    let index = gui.create_root(Handle::default(), &package, item).unwrap();
    let owner = gui.root_object(index).unwrap();
    let child = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_parent(child, Some(owner));
    gui.store_mut()
        .set_clip_softness(owner, crate::core::Point::new(24., 20.));
    (gui, owner, child)
}

#[test]
fn soft_clipping_preserves_per_draw_alpha_blend_and_owner_graphics() {
    let (mut gui, owner, child) = clip_fixture("Component1");
    gui.store_mut().set_alpha(owner, 0.5);
    gui.store_mut().set_alpha(child, 0.6);
    gui.store_mut().set_blend_mode(owner, BlendMode::Add);
    assert!(policy(&gui, owner).groups.is_empty());
    let content = policy(&gui, child);
    assert_eq!(content.groups.len(), 1);
    let clip = &content.groups[0];
    assert_eq!(clip.blend, BlendMode::Add);
    assert_eq!(clip.opacity, 1.);
    assert_eq!(clip.clip.unwrap().edges, Vec4::new(24., 20., 24., 20.));
    assert_eq!(content.blend, BlendMode::Normal);
    assert!((gui.paint_alpha(child) - 0.3).abs() < 1e-6);
    gui.store_mut()
        .set_clip_softness(owner, crate::core::Point::ZERO);
    assert!(policy(&gui, child).groups.is_empty());
}

#[test]
fn scroll_soft_edges_follow_content_and_exclude_scrollbars_and_masks() {
    let (mut gui, owner, child) = clip_fixture("Component7");
    gui.store_mut()
        .update_scroll_content_size(owner, crate::core::Size::new(1000., 1000.));
    let edges = |gui: &GuiRuntime| {
        policy(gui, child)
            .groups
            .into_iter()
            .find_map(|g| g.clip)
            .unwrap()
            .edges
    };
    assert_eq!(edges(&gui), Vec4::new(0., 0., 24., 20.));
    let max = gui.store().scroll_pane(owner).unwrap().max_scroll_pos();
    gui.store_mut()
        .set_scroll_pos(owner, max.x * 0.5, max.y * 0.5, false);
    gui.store_mut().set_scroll_softness_on_top_left(owner, true);
    assert_eq!(edges(&gui), Vec4::new(24., 20., 24., 20.));
    gui.store_mut().set_scroll_pos(owner, max.x, max.y, false);
    assert_eq!(edges(&gui), Vec4::new(24., 20., 0., 0.));
    if let Some(bar) = gui.store().scroll_pane(owner).unwrap().vt_bar {
        assert!(policy(&gui, bar).groups.iter().all(|g| g.clip.is_none()));
    }
    gui.store_mut()
        .set_component_mask(owner, Some(child), false);
    assert!(policy(&gui, child).groups.iter().all(|g| g.clip.is_none()));
}

#[test]
fn hidden_soft_edges_use_the_native_scroll_rect_bounds_after_margin_changes() {
    let (mut gui, owner, child) = clip_fixture("Component1");
    gui.store_mut().set_size(owner, 100., 80.);
    gui.store_mut()
        .set_component_margin(owner, crate::core::geom::Margin::new(12., 8., 20., 16.));
    let clip = policy(&gui, child)
        .groups
        .into_iter()
        .find_map(|g| g.clip)
        .unwrap();
    assert_eq!(
        clip.rect,
        Rect::from_corners(Vec2::ZERO, Vec2::new(80., 64.))
    );
    assert_eq!(clip.edges, Vec4::new(24., 20., 24., 20.));
}
