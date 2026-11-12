use super::*;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    old_parts: Vec<Entity>,
    foreign: Option<Handle<FuiAsset>>,
    fonts: Vec<String>,
    child: Option<Entity>,
    input: Option<GObjectId>,
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
    inline: &Query<&fairygui_bevy::runtime::InlineImageParts>,
    images: &Query<&ImageNode>,
    children: &Query<&Children>,
    spans: &Query<&TextSpan>,
    nodes: &Query<&Node>,
    font_assets: &Assets<Font>,
    server: &AssetServer,
    fonts: &Query<&TextFont>,
    layouts: &Query<(&bevy::text::ComputedTextBlock, &bevy::text::TextLayoutInfo)>,
    editables: &Query<&bevy::text::EditableText>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let foreign = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
            )
            .unwrap();
            for name in ["HitNumber", "BMFontTest"] {
                let item = foreign
                    .item(foreign.item_index_by_name(name).unwrap())
                    .unwrap();
                state
                    .fonts
                    .push(format!("ui://{}{}", foreign.id(), item.id));
            }
            state.foreign = Some(assets.add(FuiAsset {
                package: foreign,
                res_key: "ui/Basics".into(),
            }));
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let object = runtime.store().object(original).unwrap();
            let xy = Vec2::new(object.x(), object.y());
            let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
            let root = runtime.root_object(index).unwrap();
            state.root = Some(index);
            runtime.store_mut().set_xy(root, xy.x, xy.y);
            runtime.store_mut().set_visible(original, false);
            state.owners = ["n0", "n2", "n10", "n12"]
                .map(|name| runtime.store().child_by_name(root, name).unwrap())
                .to_vec();
            let all = runtime.store().object(root).unwrap().children().to_vec();
            for id in all {
                runtime
                    .store_mut()
                    .set_visible(id, state.owners.contains(&id));
            }
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_xy(
                    owner,
                    60.0 + (i % 2) as f32 * 480.0,
                    160.0 + (i / 2) as f32 * 180.0,
                );
                runtime.store_mut().set_size(owner, 400.0, 100.0);
                runtime.store_mut().set_object_ubb_enabled(owner, i == 3);
                runtime.store_mut().set_object_text(
                    owner,
                    if i == 3 {
                        "[b]Rich text[/b] [img]ui://9leh0eyfrpmb6[/img]"
                    } else {
                        "Ordinary text -> bitmap -> text"
                    },
                );
            }
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            commands
                .entity(runtime.entity_of(root).unwrap())
                .insert(GlobalZIndex(500));
            for &id in &state.owners {
                let view = runtime.entity_of(id).unwrap();
                state.views.push(view);
                if let Ok(parts) = inline.get(view) {
                    state.old_parts.extend(parts.iter());
                }
            }
            assert!(!state.old_parts.is_empty());
            state.child = Some(
                commands
                    .spawn((Node::default(), ChildOf(state.views[0])))
                    .id(),
            );
            for (i, &id) in state.owners.iter().enumerate() {
                runtime
                    .store_mut()
                    .set_object_font(id, Some(&state.fonts[i % 2]));
                runtime.store_mut().set_object_text(
                    id,
                    if i % 2 == 0 {
                        "12345"
                    } else {
                        "使用嵌入字体 UI#"
                    },
                );
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 16 => {
            assert!(state.old_parts.iter().all(|&e| images.get(e).is_err()));
            state.old_parts.clear();
            for (&id, &view) in state.owners.iter().zip(&state.views) {
                assert_eq!(runtime.entity_of(id), Some(view));
                let parts = &glyphs.get(view).unwrap().0;
                assert!(!parts.is_empty());
                state.old_parts.extend(parts);
            }
            assert!(nodes.get(state.child.unwrap()).is_ok());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-fonts-bitmap.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            for (i, &id) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_font(id, None);
                runtime.store_mut().set_object_text(
                    id,
                    if i == 3 {
                        "[color=#66AAFF]Rich text restored[/color]\n[img]ui://9leh0eyfrpmb6[/img]"
                    } else {
                        "Ordinary text restored\n保留原对象与应用子节点"
                    },
                );
            }
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 16 => {
            assert!(state.old_parts.iter().all(|&e| images.get(e).is_err()));
            assert!(state.views.iter().all(|&e| glyphs.get(e).is_err()));
            assert!(inline.get(state.views[3]).unwrap().iter().next().is_some());
            assert!(nodes.get(state.child.unwrap()).is_ok());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-fonts-restored.png"));
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 10 => {
            runtime
                .store_mut()
                .set_object_ubb_enabled(state.owners[0], true);
            runtime
                .store_mut()
                .set_object_text(state.owners[0], "[b]UBB toggled on[/b]");
            runtime
                .store_mut()
                .set_object_html_enabled(state.owners[1], true);
            runtime.store_mut().set_object_text(state.owners[1],"<font size='150%' color='#8844BB'><strike>Native &amp; HTML</strike></font><br/><a href='event:raw'>Underlined link</a> <img src='ui://9leh0eyfrpmb6' width='1' height='1'/>");
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 12 => {
            assert!(
                children
                    .get(state.views[0])
                    .unwrap()
                    .iter()
                    .any(|e| spans.get(e).is_ok())
            );
            assert!(
                children
                    .get(state.views[1])
                    .unwrap()
                    .iter()
                    .any(|e| spans.get(e).is_ok_and(|span| span.0 == "Native & HTML"))
            );
            let raw_image = inline.get(state.views[1]).unwrap().iter().next().unwrap();
            state.old_parts.push(raw_image);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-markup-html.png"));
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 10 => {
            runtime
                .store_mut()
                .set_object_ubb_enabled(state.owners[0], false);
            runtime
                .store_mut()
                .set_object_html_enabled(state.owners[1], false);
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 12 => {
            assert!(
                children
                    .get(state.views[0])
                    .unwrap()
                    .iter()
                    .all(|e| spans.get(e).is_err())
            );
            assert!(
                children.get(state.views[1]).is_err()
                    || children
                        .get(state.views[1])
                        .unwrap()
                        .iter()
                        .all(|e| spans.get(e).is_err())
            );
            assert!(nodes.get(state.child.unwrap()).is_ok());
            state
                .old_parts
                .extend(inline.get(state.views[3]).unwrap().iter());
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_font(
                    owner,
                    Some(
                        [
                            "Times New Roman",
                            "fonts/FiraMono-subset.ttf",
                            "fonts/NotoSansSC-VF.ttf",
                            "fonts/FiraMono-subset.ttf",
                        ][i],
                    ),
                );
                runtime.store_mut().set_object_font_size(owner, 30);
                runtime.store_mut().set_object_text(
                    owner,
                    [
                        "Family: Times New Roman\n0123456789",
                        "File: Fira Mono\n0123456789",
                        "File: Noto Sans SC\n从字体文件加载",
                        "[b]Rich file font[/b]\n[color=#66AAFF]Fira Mono 012345[/color]",
                    ][i],
                );
            }
            let input = runtime
                .store_mut()
                .create(fairygui_core::package::ObjectType::InputText);
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            runtime.store_mut().set_parent(input, Some(root));
            runtime.store_mut().set_xy(input, 60.0, 450.0);
            runtime.store_mut().set_size(input, 700.0, 50.0);
            runtime.store_mut().set_control_color(
                input,
                fairygui_core::color::Color::from_rgba8(20, 70, 110, 255),
                None,
            );
            runtime.store_mut().set_object_font_size(input, 30);
            runtime
                .store_mut()
                .set_object_font(input, Some("fonts/FiraMono-subset.ttf"));
            runtime
                .store_mut()
                .set_object_text(input, "Editable file font: 0123456789");
            runtime.queue_subtree(input);
            state.input = Some(input);
            state.stage = 20;
            state.since = frames;
        }
        20 if frames - state.since >= 16 => {
            use bevy::text::FontSource;
            let file: Handle<Font> = server.get_handle("fonts/FiraMono-subset.ttf").unwrap();
            let chinese: Handle<Font> = server.get_handle("fonts/NotoSansSC-VF.ttf").unwrap();
            if !font_assets.contains(&file) || !font_assets.contains(&chinese) {
                assert!(
                    frames - state.since < 300,
                    "raster fonts did not finish loading"
                );
                return;
            }
            assert!(
                matches!(&fonts.get(state.views[0]).unwrap().font, FontSource::List(sources) if sources[0]==FontSource::families("Times New Roman"))
            );
            let (family_block, family_info) = layouts.get(state.views[0]).unwrap();
            assert!(!family_info.glyphs.is_empty());
            if cfg!(target_os = "windows") {
                let fallback_data = font_assets.get(&chinese).unwrap().data.id();
                assert!(
                    family_block
                        .buffer()
                        .lines()
                        .flat_map(|line| line.runs())
                        .all(|run| run.font().data.id() != fallback_data),
                    "installed family must select a different face from the configured Noto Sans SC fallback"
                );
            }
            for (i, &view) in state.views.iter().enumerate().skip(1) {
                let handle = if i == 2 { &chinese } else { &file };
                assert_eq!(
                    fonts.get(view).unwrap().font,
                    FontSource::Handle(handle.clone())
                );
                let (block, info) = layouts.get(view).unwrap();
                assert!(!info.glyphs.is_empty());
                let expected = font_assets.get(handle).unwrap().data.id();
                assert!(
                    block
                        .buffer()
                        .lines()
                        .flat_map(|line| line.runs())
                        .all(|run| run.font().data.id() == expected),
                    "actual shaped runs must use the requested file"
                );
            }
            let input = runtime.entity_of(state.input.unwrap()).unwrap();
            assert_eq!(fonts.get(input).unwrap().font, FontSource::Handle(file));
            assert_eq!(
                editables.get(input).unwrap().value().to_string(),
                "Editable file font: 0123456789"
            );
            assert!(!layouts.get(input).unwrap().1.glyphs.is_empty());
            state.views.push(input);
            assert!(nodes.get(state.child.unwrap()).is_ok());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-raster-fonts.png"));
            state.stage = 21;
            state.since = frames;
        }
        21 if frames - state.since >= 10 => {
            for &owner in &state.owners {
                runtime
                    .store_mut()
                    .set_object_font(owner, Some("fonts/NotoSansSC-VF.ttf"));
            }
            runtime
                .store_mut()
                .set_object_font(state.input.unwrap(), Some("fonts/NotoSansSC-VF.ttf"));
            runtime
                .store_mut()
                .set_object_text(state.input.unwrap(), "Updated editor: 保留输入缓冲");
            state.stage = 22;
            state.since = frames;
        }
        22 if frames - state.since >= 16 => {
            let chinese: Handle<Font> = server.get_handle("fonts/NotoSansSC-VF.ttf").unwrap();
            for &view in &state.views {
                assert_eq!(
                    fonts.get(view).unwrap().font,
                    bevy::text::FontSource::Handle(chinese.clone())
                );
                assert!(
                    !layouts.get(view).unwrap().1.glyphs.is_empty(),
                    "raster font view {view:?}"
                );
            }
            assert_eq!(
                editables
                    .get(*state.views.last().unwrap())
                    .unwrap()
                    .value()
                    .to_string(),
                "Updated editor: 保留输入缓冲"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-raster-fonts-updated.png",
                ));
            state.stage = 23;
            state.since = frames;
        }
        23 if frames - state.since >= 10 => {
            runtime.remove_root(commands, state.root.unwrap());
            runtime.store_mut().set_visible(original, true);
            state.foreign = None;
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| nodes.get(e).is_err()));
            assert!(state.old_parts.iter().all(|&e| images.get(e).is_err()));
            state.stage = 9;
            println!(
                "demo smoke: ordinary/rich text switched to foreign image/atlas fonts and back; native HTML/UBB, real TTF loading, family selection, actual file-backed shaped runs, runtime raster font changes, editable file text, retained children and disposal verified"
            );
        }
        _ => {}
    }
}
