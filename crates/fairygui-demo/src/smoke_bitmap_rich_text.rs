use super::*;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    roots: Vec<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    parts: Vec<Entity>,
    glyphs: Vec<Vec<Entity>>,
    labels: Vec<Entity>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 9
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    glyphs: &Query<&fairygui_bevy::runtime::BitmapGlyphs>,
    images: &Query<&ImageNode>,
    children: &Query<&Children>,
    nodes: &Query<&Node>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let page = runtime.store().object(original).unwrap();
            let origin = Vec2::new(page.x() + 50.0, page.y() + 125.0);
            runtime.store_mut().set_visible(original, false);
            state.labels.push(
                commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(origin.x),
                            top: Val::Px(origin.y),
                            width: Val::Px(1000.0),
                            height: Val::Px(480.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        GlobalZIndex(500),
                    ))
                    .id(),
            );
            for (i, name) in ["n0", "n7", "n10", "n8", "n2", "n4"].iter().enumerate() {
                let index = runtime
                    .create_root(handle.clone(), &package, "Demo_Text")
                    .unwrap();
                let root = runtime.root_object(index).unwrap();
                let owner = runtime.store().child_by_name(root, name).unwrap();
                let all = runtime.store().object(root).unwrap().children().to_vec();
                for child in all {
                    runtime.store_mut().set_visible(child, child == owner);
                }
                let xy = origin
                    + Vec2::new(25.0 + (i % 3) as f32 * 320.0, 15.0 + (i / 3) as f32 * 235.0);
                runtime.store_mut().set_xy(root, xy.x, xy.y + 28.0);
                runtime.store_mut().set_xy(owner, 0.0, 0.0);
                runtime.store_mut().set_size(owner, 270.0, 165.0);
                runtime.store_mut().set_object_ubb_enabled(owner, i != 3);
                runtime.store_mut().set_object_html_enabled(owner, i == 3);
                runtime.store_mut().set_control_color(
                    owner,
                    fairygui_core::color::Color::from_rgba8(35, 55, 75, 255),
                    None,
                );
                runtime.store_mut().set_object_text(owner,if i==3 {
                    "<font size='150%' color='#22AA88'><strike>UI&#35;</strike></font><br/><img src='ui://bmfont01rpmb6'/><a href='event:raw'>使用字体</a>"
                } else if i<3 {
                    "1[size=40][color=#22AA88]2[/color][/size]\n[u]3[/u][img]ui://bmfont01rpmb6[/img][url=event:bitmap]45[/url]"
                } else {
                    "U[size=48][color=#22AA88]I[/color][/size]\n[u]#[/u][img]ui://bmfont01rpmb6[/img][url=event:bitmap]使用字体[/url]"
                });
                state.labels.push(
                    commands
                        .spawn((
                            Text::new(
                                [
                                    "Color / fixed",
                                    "Color / ellipsis",
                                    "Color / auto both",
                                    "HTML / size / strike",
                                    "Tinted / auto both",
                                    "Tinted / shrink",
                                ][i],
                            ),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::BLACK),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(xy.x),
                                top: Val::Px(xy.y),
                                ..default()
                            },
                            GlobalZIndex(502),
                        ))
                        .id(),
                );
                state.roots.push(index);
                state.owners.push(owner);
            }
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            for (i, &owner) in state.owners.iter().enumerate() {
                let view = runtime.entity_of(owner).unwrap();
                let gs = glyphs.get(view).unwrap().0.clone();
                assert!(!gs.is_empty());
                let parts: Vec<_> = children.get(view).unwrap().iter().collect();
                if i != 1 {
                    assert!(
                        parts
                            .iter()
                            .any(|e| !gs.contains(e) && images.get(*e).is_ok()),
                        "bitmap inline image must be drawn"
                    );
                }
                state.parts.extend(parts);
                state.glyphs.push(gs);
                state.views.push(view);
                let root = runtime.root_object(state.roots[i]).unwrap();
                commands
                    .entity(runtime.entity_of(root).unwrap())
                    .insert(GlobalZIndex(501));
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-bitmap-rich.png"));
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 10 => {
            for &id in &state.owners {
                runtime.store_mut().set_grayed(id, true);
            }
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 12 => {
            for (&view, gs) in state.views.iter().zip(&state.glyphs) {
                assert_eq!(&glyphs.get(view).unwrap().0, gs);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-bitmap-rich-gray.png"));
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 10 => {
            for (i, &id) in state.owners.iter().enumerate() {
                runtime.store_mut().set_grayed(id, false);
                runtime.store_mut().set_size(id, 130.0, 160.0);
                runtime.store_mut().set_object_font_size(id, 30);
                runtime.store_mut().set_object_text(id,if i==3 {"<strike><font color='#BB4499' size='150%'>UI&#35;</font></strike><br/><img src='ui://bmfont01rpmb6'/><a href='event:raw'>使用嵌入字体</a>"}else if i<3 {"[u]1[/u][size=40][color=#BB4499]2345[/color][/size]\n[img]ui://bmfont01rpmb6[/img]678"}else{"[u]U[/u][size=48][color=#BB4499]I#[/color][/size]\n[img]ui://bmfont01rpmb6[/img]使用嵌入字体"});
            }
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 16 => {
            assert!(
                state
                    .parts
                    .iter()
                    .all(|&e| children.get(e).is_err() && nodes.get(e).is_err())
            );
            assert!(
                state
                    .owners
                    .iter()
                    .zip(&state.views)
                    .all(|(&id, &view)| runtime.entity_of(id) == Some(view))
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-bitmap-rich-updated.png"));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            state.parts.clear();
            for &view in &state.views {
                state.parts.extend(children.get(view).unwrap().iter());
            }
            for &index in &state.roots {
                runtime.remove_root(commands, index);
            }
            for &view in &state.labels {
                commands.entity(view).despawn();
            }
            runtime.store_mut().set_visible(original, true);
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| nodes.get(e).is_err()));
            assert!(
                state
                    .parts
                    .iter()
                    .all(|&e| nodes.get(e).is_err() && images.get(e).is_err())
            );
            state.stage = 9;
            println!(
                "demo smoke: bitmap UBB size/color/underline/link and native inline images, image/atlas fonts, fixed/auto/shrink/ellipsis, stable gray, resize/rebuild and disposal verified"
            );
        }
        _ => {}
    }
}
