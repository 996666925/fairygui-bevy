use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

fn fixture(
    source: &str,
    rich: bool,
    multiline: bool,
    loaded: bool,
) -> (App, Entity, Entity, Handle<Font>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let font = app.world().resource::<Assets<Font>>().reserve_handle();
    if loaded {
        load(&mut app, &font);
    }
    app.insert_resource(GuiFont(Some(font.clone())));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "TextTemplate",
                include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
            )
            .unwrap(),
            res_key: "ui/TextTemplate".into(),
        });
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset, "Label"),
            GuiText::new(source),
            GuiTextFormat {
                font_size: 24,
                leading: 3,
                auto_size: 4,
                single_line: !multiline,
                ubb_enabled: rich,
                html_enabled: rich,
                ..default()
            },
        ))
        .id();
    app.update();
    let title = child(&mut app, owner, "title");
    (app, owner, title, font)
}
fn load(app: &mut App, handle: &Handle<Font>) {
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            handle.id(),
            Font::from_bytes(
                include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
            ),
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
fn visible(app: &App, title: Entity) -> &str {
    &app.world()
        .get::<EllipsisState>(title)
        .unwrap()
        .projected
        .as_ref()
        .unwrap()
        .2
}

fn link_at_run(app: &App, title: Entity, index: usize) -> Option<String> {
    let info = app.world().get::<TextLayoutInfo>(title).unwrap();
    let block = app.world().get::<ComputedTextBlock>(title).unwrap();
    let computed = ComputedNode {
        size: app.world().get::<GuiTransform>(title).unwrap().size(),
        inverse_scale_factor: 1.,
        padding: bevy::sprite::BorderRect {
            min_inset: Vec2::splat(2.),
            max_inset: Vec2::splat(2.),
        },
        ..default()
    };
    let point = (info.run_geometry[index].bounds.center() + computed.content_inset().min_inset)
        / computed.size();
    app.world()
        .get::<crate::runtime::RichTextBlock>(title)
        .unwrap()
        .link_at(
            app.world().non_send::<GuiRuntime>(),
            app.world().get::<GObjectRef>(title).unwrap().0,
            point,
            &computed,
            block,
            info,
        )
        .map(|hit| hit.href)
}

#[test]
fn first_frame_single_line_keeps_original_unicode_and_restores_on_resize_or_mode_change() {
    const SOURCE: &str = "Unicode 中文😀🧚 long text that must be shortened";
    let (mut app, owner, title, _) = fixture(SOURCE, false, false, true);
    assert!(visible(&app, title).ends_with('…'));
    assert!(!visible(&app, title).contains('\u{fffd}'));
    assert!(!visible(&app, title).contains('\n'));
    assert_eq!(app.world().get::<GuiText>(owner).unwrap().value(), SOURCE);
    assert_eq!(app.world().get::<Text>(title).unwrap().0, SOURCE);
    assert_eq!(
        app.world().get::<TextFont>(title).unwrap().font_size,
        FontSize::Px(24.)
    );
    assert_eq!(
        app.world()
            .get::<ComputedTextBlock>(title)
            .unwrap()
            .buffer()
            .lines()
            .count(),
        1
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(1600., 60.));
    app.update();
    assert!(
        !app.world()
            .get::<EllipsisState>(title)
            .unwrap()
            .is_projected()
    );
    assert_eq!(app.world().get::<Text>(title).unwrap().0, SOURCE);
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(140., 40.));
    app.update();
    assert!(visible(&app, title).ends_with('…'));
    app.world_mut()
        .get_mut::<GuiTextFormat>(owner)
        .unwrap()
        .auto_size = 0;
    app.update();
    assert!(app.world().get::<EllipsisState>(title).is_none());
    assert_eq!(app.world().get::<Text>(title).unwrap().0, SOURCE);
}

#[test]
fn multiline_freezes_visible_breaks_and_keeps_link_styles_and_hidden_links_out_of_geometry() {
    let source = "[url=front][color=#ff0000]Front 中文😀 text text text[/color][/url]\n[url=hidden]Hidden line behind the limit[/url]";
    let (mut app, _, title, _) = fixture(source, true, true, true);
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(130., 55.));
    app.update();
    let shown = visible(&app, title);
    assert!(shown.ends_with('…'));
    assert!(!shown.contains("Hidden"));
    let block = app.world().get::<ComputedTextBlock>(title).unwrap();
    assert!(block.buffer().lines().count() <= 2);
    let info = app.world().get::<TextLayoutInfo>(title).unwrap();
    assert!(!info.glyphs.is_empty());
    for run in &info.run_geometry {
        let span = block.entities()[run.section_index as usize].entity;
        if let Some(text) = app.world().get::<TextSpan>(span) {
            assert!(!text.0.contains("Hidden"));
            assert!(app.world().get::<Underline>(span).is_some());
            assert_eq!(
                app.world().get::<TextColor>(span).unwrap().0,
                Color::srgb(1., 0., 0.)
            );
        }
    }
    assert_eq!(link_at_run(&app, title, 0).as_deref(), Some("front"));
}

#[test]
fn clipped_image_uses_a_styled_marker_without_resizing_or_deleting_the_original_box() {
    let (mut app, _, title, _) = fixture(
        "<font color='#FF0000'>A</font><a href='image'><img src='missing.png' width='12' height='10'/></a> trailing text beyond the field",
        true,
        false,
        true,
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(36., 45.));
    app.update();
    let state = app.world().get::<EllipsisState>(title).unwrap();
    assert!(!state.markers.is_empty(), "{}", visible(&app, title));
    let target = state.markers[0].0;
    assert_eq!(
        app.world()
            .get::<bevy::text::InlineBox>(target)
            .unwrap()
            .size,
        Vec2::new(14., 10.)
    );
    assert!(
        app.world()
            .get::<TextLayoutInfo>(title)
            .unwrap()
            .inline_boxes
            .iter()
            .all(|(entity, _, _)| *entity != target)
    );
    assert_eq!(
        app.world().get::<TextColor>(target).unwrap().0,
        Color::srgb(1., 0., 0.)
    );
    let parts = app
        .world()
        .get::<crate::runtime::InlineImageParts>(title)
        .unwrap();
    for image in parts.iter() {
        assert_eq!(
            app.world().get::<Node>(image).unwrap().display,
            Display::None
        );
    }
    let last = app
        .world()
        .get::<TextLayoutInfo>(title)
        .unwrap()
        .run_geometry
        .len()
        - 1;
    assert_eq!(link_at_run(&app, title, last).as_deref(), Some("image"));
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(1600., 200.));
    app.update();
    assert!(
        app.world()
            .get::<TextLayoutInfo>(title)
            .unwrap()
            .inline_boxes
            .iter()
            .any(|(entity, _, _)| *entity == target)
    );
}

#[test]
fn late_fonts_and_same_frame_source_changes_recompute_without_changing_requested_size() {
    let (mut app, owner, title, font) =
        fixture("Late font text that needs clipping", false, false, false);
    assert!(app.world().get::<EllipsisState>(title).is_none());
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(140., 45.));
    load(&mut app, &font);
    app.update();
    assert!(
        app.world()
            .get::<EllipsisState>(title)
            .unwrap()
            .is_projected(),
        "late font arrival should enable the overflow projection"
    );
    app.world_mut()
        .get_mut::<GuiText>(owner)
        .unwrap()
        .set("short");
    app.update();
    assert!(
        !app.world()
            .get::<EllipsisState>(title)
            .unwrap()
            .is_projected()
    );
    app.world_mut()
        .get_mut::<GuiText>(owner)
        .unwrap()
        .set("A different long string 中文 that needs clipping again");
    app.update();
    assert!(visible(&app, title).starts_with("A "));
    assert_eq!(
        app.world().get::<GuiTextFormat>(owner).unwrap().font_size,
        24
    );
    let images = app.world().resource::<Assets<Image>>().len();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().resource::<Assets<Image>>().len(), images);
}

#[test]
fn input_focus_password_and_prompt_projection_never_replace_the_editable_value() {
    let (mut app, owner, _, _) = fixture("label", false, false, true);
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let input = child(&mut app, owner, "input");
    const SOURCE: &str = "Input 中文😀 with a long suffix";
    app.world_mut().entity_mut(input).insert((
        GuiText::new(SOURCE),
        GuiTextFormat {
            font_size: 24,
            auto_size: 4,
            single_line: true,
            ..default()
        },
    ));
    app.world_mut()
        .gui_mut(input)
        .unwrap()
        .set_size(Vec2::new(120., 38.));
    app.update();
    assert!(
        app.world()
            .get::<EllipsisState>(input)
            .unwrap()
            .is_projected()
    );
    assert_eq!(
        app.world().get::<EditableText>(input).unwrap().value(),
        SOURCE
    );
    let mut input_state = SystemState::<GuiInput>::new(app.world_mut());
    assert!(input_state.get_mut(app.world_mut()).unwrap().focus(input));
    app.update();
    assert!(app.world().get::<EllipsisState>(input).is_none());
    assert_eq!(
        app.world().get::<EditableText>(input).unwrap().value(),
        SOURCE
    );
    input_state.get_mut(app.world_mut()).unwrap().blur();
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .password = true;
    app.update();
    assert!(visible(&app, input).ends_with('…'));
    assert!(visible(&app, input).chars().all(|c| c == '●' || c == '…'));
    assert_eq!(
        app.world().get::<EditableText>(input).unwrap().value(),
        SOURCE
    );
    app.world_mut()
        .get_mut::<GuiTextInputSettings>(input)
        .unwrap()
        .prompt_text = Some("Prompt {literal} text with a long suffix".into());
    app.world_mut().get_mut::<GuiText>(input).unwrap().set("");
    app.update();
    let prompt = app
        .world_mut()
        .query_filtered::<Entity, With<InputPromptOwner>>()
        .iter(app.world())
        .find(|&entity| {
            app.world()
                .get::<ChildOf>(entity)
                .is_some_and(|parent| parent.parent() == input)
        })
        .unwrap();
    assert!(
        app.world()
            .get::<EllipsisState>(prompt)
            .unwrap()
            .is_projected()
    );
    assert_eq!(
        app.world().get::<Text>(prompt).unwrap().0,
        "Prompt {literal} text with a long suffix"
    );
    assert_eq!(app.world().get::<EditableText>(input).unwrap().value(), "");
}

#[test]
fn mixed_direction_spans_keep_the_logical_source_prefix() {
    let (mut app, _, title, _) = fixture(
        "[url=rtl]אבגדהוזחט[/url] Latin suffix with many words",
        true,
        false,
        true,
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(140., 45.));
    app.update();
    let shown = visible(&app, title).trim_end_matches('…');
    assert!(
        "אבגדהוזחט Latin suffix with many words".starts_with(shown),
        "{shown}"
    );
    assert!(!shown.is_empty());
}

#[test]
fn image_only_markers_inherit_the_field_underline_and_retain_the_image_link() {
    let (mut app, owner, title, _) = fixture(
        "<a href='only'><img src='missing.png' width='200' height='40'/></a>",
        true,
        false,
        true,
    );
    app.world_mut()
        .get_mut::<GuiTextFormat>(owner)
        .unwrap()
        .underline = true;
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(40., 45.));
    app.update();
    assert_eq!(visible(&app, title), "…");
    let state = app.world().get::<EllipsisState>(title).unwrap();
    assert!(app.world().get::<Underline>(state.markers[0].0).is_some());
    assert_eq!(link_at_run(&app, title, 0).as_deref(), Some("only"));
}
