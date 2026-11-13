//! Exercise text formatting using only public ECS components and observers.
use super::*;
use bevy::text::{EditableText, LetterSpacing, LineHeight};
use fairygui_bevy::runtime::BitmapGlyphs;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    views: Vec<Entity>,
    child: Option<Entity>,
    font_asset: Option<Handle<FuiAsset>>,
    font_url: String,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Clicks(u32);
fn clicked(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click && !event.event.bubbling {
        clicks.0 += 1;
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Clicks>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    clicks: Res<Clicks>,
    demo: Res<Demo>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<(&mut GuiTextFormat, &mut GuiText)>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
    )>,
    assets: Res<AssetServer>,
    mut packages: ResMut<Assets<FuiAsset>>,
    fonts: Query<(&TextFont, &LineHeight, &LetterSpacing)>,
    (glyphs, images, image_assets): (Query<&BitmapGlyphs>, Query<&ImageNode>, Res<Assets<Image>>),
    editables: Query<&EditableText>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    entities: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "text-format smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
                return;
            };
            ui.p3().get_mut(menu).unwrap().visible = false;
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Text")
                            .with_position(Vec2::new(70., 80.)),
                    )
                    .id(),
            );
            // Preserve the original atlas filename; select the other package
            // by its distinct ID rather than its deliberately reused name.
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
            )
            .unwrap();
            let item = package
                .item(package.item_index_by_name("HitNumber").unwrap())
                .unwrap();
            state.font_url = format!("ui://{}{}", package.id(), item.id);
            state.font_asset = Some(packages.add(FuiAsset {
                package,
                res_key: "ui/Basics".into(),
            }));
            next = true;
        }
        1 if elapsed >= 16 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let (views, children) = {
                let gui = ui.p0();
                (
                    ["n2", "n12", "n22", "n25"].map(|name| gui.child_by_name(owner, name).unwrap()),
                    gui.get(owner).unwrap().children().to_vec(),
                )
            };
            for child in children {
                ui.p3().get_mut(child).unwrap().visible = views.contains(&child);
            }
            for (i, &view) in views.iter().enumerate() {
                {
                    let mut transforms = ui.p2();
                    let mut transform = transforms.get_mut(view).unwrap();
                    transform.position =
                        Vec2::new(30. + (i % 2) as f32 * 540., 30. + (i / 2) as f32 * 200.);
                    transform.size = Vec2::new(440., 110.);
                }
                let mut edits = ui.p1();
                let (mut format, mut text) = edits.get_mut(view).unwrap();
                format.font = None;
                format.font_size = 32;
                format.color = [
                    Color::srgb(0.2, 0.6, 1.),
                    Color::srgb(1., 0.6, 0.2),
                    Color::srgb(0.3, 0.9, 0.4),
                    Color::srgb(0.4, 0.1, 0.7),
                ][i];
                format.bold = true;
                format.italic = i == 0;
                format.underline = i < 2;
                format.auto_size = 0;
                format.align = 1;
                format.vertical_align = 1;
                format.leading = 8;
                format.letter_spacing = 2;
                format.stroke = if i == 0 { 1. } else { 0. };
                format.stroke_color = Color::BLACK;
                format.ubb_enabled = i == 1;
                format.html_enabled = i == 1;
                text.set(
                    [
                        "ECS text 中文",
                        "[b]Rich text[/b]\n[color=#66ccff]富文本样式[/color]",
                        "123",
                        "Click styled button",
                    ][i],
                );
            }
            state.views = views.to_vec();
            state.child = Some(commands.spawn((Node::default(), ChildOf(views[2]))).id());
            commands.entity(views[3]).observe(clicked);
            next = true;
        }
        2 if elapsed >= 16 => {
            let (font, leading, spacing) = fonts.get(state.views[0]).unwrap();
            assert_eq!(font.font_size, FontSize::Px(32.));
            assert_eq!(font.weight, FontWeight::BOLD);
            assert_eq!(font.style, FontStyle::Italic);
            assert_eq!(*leading, LineHeight::Px(40.));
            assert_eq!(*spacing, LetterSpacing::Px(2.));
            assert!(editables.get(state.views[2]).is_ok());
            let title = ui.p0().child_by_name(state.views[3], "title").unwrap();
            assert_eq!(fonts.get(title).unwrap().0.font_size, FontSize::Px(32.));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-format.png"));
            next = true;
        }
        3 if elapsed >= 6 => {
            for index in [0, 2] {
                let mut edits = ui.p1();
                let (mut format, mut text) = edits.get_mut(state.views[index]).unwrap();
                format.font = Some(state.font_url.clone());
                format.italic = false;
                format.stroke = 0.;
                format.font_size = 42;
                text.set("12345");
            }
            next = true;
        }
        4 if elapsed >= 16 => {
            for index in [0, 2] {
                let parts = &glyphs.get(state.views[index]).unwrap().0;
                assert!(!parts.is_empty());
                for &part in parts {
                    let image = images.get(part).unwrap();
                    assert!(
                        image_assets.get(&image.image).is_some(),
                        "bitmap glyph atlas is not loaded"
                    );
                }
            }
            assert!(editables.get(state.views[2]).is_err());
            assert!(entities.contains(state.child.unwrap()));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-format-bitmap.png"));
            next = true;
        }
        5 if elapsed >= 6 => {
            for index in [0, 2] {
                let mut edits = ui.p1();
                let (mut format, mut text) = edits.get_mut(state.views[index]).unwrap();
                format.font = None;
                format.font_size = 36;
                format.underline = false;
                text.set(if index == 0 {
                    "Restored 中文"
                } else {
                    "12345"
                });
            }
            next = true;
        }
        6 if elapsed >= 16 => {
            assert!(editables.get(state.views[2]).is_ok());
            assert!(glyphs.get(state.views[0]).is_err());
            assert!(entities.contains(state.child.unwrap()));
            assert_eq!(ui.p0().text(state.views[2]), Some("12345"));
            let (_, transform) = nodes.get(state.views[3]).unwrap();
            state.cursor = Some(transform.transform_point2(Vec2::ZERO) / window.scale_factor());
            next = true;
        }
        7 if elapsed >= 6 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        8 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        9 if elapsed >= 5 => {
            assert_eq!(clicks.0, 1);
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        10 if elapsed >= 6 => {
            assert!(ui.p0().view(state.owner.unwrap()).is_none());
            assert!(state.views.iter().all(|&view| !entities.contains(view)));
            assert!(!entities.contains(state.child.unwrap()));
            println!(
                "demo smoke PASSED: ECS text format, title forwarding, ordinary/rich/bitmap input rendering, retained values and application children, real styled-button click and disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}
