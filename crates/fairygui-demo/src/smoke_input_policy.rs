use super::*;
use bevy::text::{EditableText, TextEdit};
use fairygui_core::FuiPackage;

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    labels: Vec<Entity>,
    prompts: Vec<Entity>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 8
    }
}
fn queue(commands: &mut Commands, entity: Entity, edits: Vec<TextEdit>) {
    commands.queue(move |world: &mut World| {
        world
            .get_mut::<EditableText>(entity)
            .unwrap()
            .pending_edits
            .extend(edits);
    });
}
#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    editors: &Query<&EditableText>,
    layouts: &Query<(&bevy::text::ComputedTextBlock, &bevy::text::TextLayoutInfo)>,
    nodes: &Query<&Node>,
    prompts: &Query<(
        Entity,
        &fairygui_bevy::runtime::InputPromptOwner,
        &Visibility,
    )>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let page = runtime.store().object(original).unwrap();
            let xy = Vec2::new(page.x(), page.y());
            let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, xy.x, xy.y);
            for child in runtime.store().object(root).unwrap().children().to_vec() {
                runtime.store_mut().set_visible(child, false);
            }
            runtime.store_mut().set_visible(original, false);
            for i in 0..3 {
                let owner = runtime
                    .store_mut()
                    .create(fairygui_core::package::ObjectType::InputText);
                runtime.store_mut().set_parent(owner, Some(root));
                runtime
                    .store_mut()
                    .set_xy(owner, 90.0, 200.0 + i as f32 * 110.0);
                runtime.store_mut().set_size(owner, 900.0, 80.0);
                runtime.store_mut().set_object_font_size(owner, 26);
                runtime.store_mut().set_object_single_line(owner, i != 1);
                runtime
                    .store_mut()
                    .set_input_max_length(owner, [6, 5, 8][i]);
                runtime
                    .store_mut()
                    .set_input_restrict(owner, [Some("0-9"), None, Some("^0-9")][i])
                    .unwrap();
                runtime.store_mut().set_control_color(
                    owner,
                    fairygui_core::color::Color::from_rgba8(20, 70, 110, 255),
                    None,
                );
                runtime.queue_subtree(owner);
                state.owners.push(owner);
                state.labels.push(
                    commands
                        .spawn((
                            Text::new(
                                [
                                    "Digits / UTF-16 max 6",
                                    "Multiline / UTF-16 max 5",
                                    "Exclude digits / IME commit",
                                ][i],
                            ),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(18.),
                                ..default()
                            },
                            TextColor(Color::BLACK),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(xy.x + 90.),
                                top: Val::Px(xy.y + 170. + i as f32 * 110.),
                                ..default()
                            },
                            GlobalZIndex(652),
                        ))
                        .id(),
                );
            }
            state.root = Some(index);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            let root = runtime.root_object(state.root.unwrap()).unwrap();
            commands
                .entity(runtime.entity_of(root).unwrap())
                .insert(GlobalZIndex(650));
            state.views = state
                .owners
                .iter()
                .map(|&id| runtime.entity_of(id).unwrap())
                .collect();
            for (i, &entity) in state.views.iter().enumerate() {
                queue(
                    commands,
                    entity,
                    vec![TextEdit::Insert(
                        ["a12b34567", "A\n中😀", "abc12Z"][i].into(),
                    )],
                );
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 14 => {
            for (i, &view) in state.views.iter().enumerate() {
                assert_eq!(
                    editors.get(view).unwrap().value().to_string(),
                    ["1234", "A\n中😀", "abcZ"][i]
                );
                assert!(!layouts.get(view).unwrap().1.glyphs.is_empty());
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-input-policy.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            runtime
                .store_mut()
                .set_object_text(state.owners[0], "Programmatic ABC123456789");
            runtime
                .store_mut()
                .set_object_single_line(state.owners[1], true);
            runtime.store_mut().set_input_max_length(state.owners[1], 6);
            runtime
                .store_mut()
                .set_input_restrict(state.owners[1], Some("0-9"))
                .unwrap();
            commands.queue(|world: &mut World| {
                world
                    .resource_mut::<bevy::clipboard::Clipboard>()
                    .set_text("a9b8C7")
                    .unwrap();
            });
            queue(
                commands,
                state.views[1],
                vec![TextEdit::SelectAll, TextEdit::Paste],
            );
            queue(
                commands,
                state.views[2],
                vec![
                    TextEdit::SelectAll,
                    TextEdit::ImeSetCompose {
                        value: "123文".into(),
                        cursor: None,
                    },
                ],
            );
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 12 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "Programmatic ABC123456789"
            );
            assert_eq!(
                editors.get(state.views[1]).unwrap().value().to_string(),
                "987"
            );
            assert!(editors.get(state.views[2]).unwrap().is_composing());
            queue(
                commands,
                state.views[2],
                vec![TextEdit::ImeCommit {
                    value: "A1中2B".into(),
                }],
            );
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 12 => {
            assert_eq!(
                editors.get(state.views[2]).unwrap().value().to_string(),
                "A中B"
            );
            assert!(!editors.get(state.views[2]).unwrap().is_composing());
            for (i, &view) in state.views.iter().enumerate() {
                assert!(!layouts.get(view).unwrap().1.glyphs.is_empty());
                assert_eq!(runtime.entity_of(state.owners[i]), Some(view));
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-policy-updated.png",
                ));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            runtime
                .store_mut()
                .set_input_restrict(state.owners[0], None)
                .unwrap();
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_text(owner, "");
                runtime.store_mut().set_input_prompt(
                    owner,
                    Some(
                        [
                            "[color=red][b]请输入名字[/b][/color]",
                            "Read-only prompt",
                            "[u]UI#[/u]",
                        ][i],
                    ),
                );
                runtime.store_mut().set_input_editable(owner, i != 1);
                if i == 2 {
                    runtime
                        .store_mut()
                        .set_object_font(owner, Some("BMFontTest"));
                }
                commands.entity(state.labels[i]).insert(Text::new(
                    [
                        "Empty / stripped UBB / default gray",
                        "Read only / selection and copy",
                        "Bitmap input / tinted prompt",
                    ][i],
                ));
            }
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 16 => {
            state.prompts = state
                .owners
                .iter()
                .map(|owner| prompts.iter().find(|(_, o, _)| o.0 == *owner).unwrap().0)
                .collect();
            assert!(
                state
                    .prompts
                    .iter()
                    .all(|&e| *prompts.get(e).unwrap().2 == Visibility::Inherited)
            );
            assert!(
                editors
                    .get(state.views[0])
                    .unwrap()
                    .value()
                    .to_string()
                    .is_empty()
            );
            assert!(
                editors
                    .get(state.views[1])
                    .unwrap()
                    .value()
                    .to_string()
                    .is_empty()
            );
            assert!(editors.get(state.views[2]).is_err());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-input-prompts.png"));
            queue(
                commands,
                state.views[1],
                vec![
                    TextEdit::Insert("REJECTED".into()),
                    TextEdit::Paste,
                    TextEdit::ImeCommit {
                        value: "REJECTED".into(),
                    },
                ],
            );
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 10 => {
            assert!(
                editors
                    .get(state.views[1])
                    .unwrap()
                    .value()
                    .to_string()
                    .is_empty()
            );
            queue(
                commands,
                state.views[0],
                vec![TextEdit::ImeSetCompose {
                    value: "预".into(),
                    cursor: None,
                }],
            );
            state.stage = 12;
            state.since = frames;
        }
        12 if frames - state.since >= 10 => {
            assert_eq!(
                *prompts.get(state.prompts[0]).unwrap().2,
                Visibility::Hidden
            );
            queue(
                commands,
                state.views[0],
                vec![TextEdit::ImeCommit {
                    value: "输入值".into(),
                }],
            );
            runtime.store_mut().set_input_prompt_color(
                state.owners[1],
                fairygui_core::color::Color::from_rgba8(40, 100, 180, 255),
            );
            runtime
                .store_mut()
                .set_object_font_size(state.owners[1], 32);
            runtime.store_mut().set_object_text(state.owners[2], "UI#");
            state.stage = 13;
            state.since = frames;
        }
        13 if frames - state.since >= 14 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "输入值"
            );
            assert_eq!(
                *prompts.get(state.prompts[0]).unwrap().2,
                Visibility::Hidden
            );
            let custom = prompts
                .iter()
                .find(|(_, o, _)| o.0 == state.owners[1])
                .unwrap()
                .0;
            assert!(nodes.get(state.prompts[1]).is_err());
            state.prompts[1] = custom;
            assert_eq!(*prompts.get(custom).unwrap().2, Visibility::Inherited);
            assert_eq!(
                *prompts.get(state.prompts[2]).unwrap().2,
                Visibility::Hidden
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-prompts-updated.png",
                ));
            state.stage = 15;
            state.since = frames;
        }
        15 if frames - state.since >= 10 => {
            runtime
                .store_mut()
                .set_input_editable(state.owners[1], true);
            queue(commands, state.views[1], vec![TextEdit::Insert("8".into())]);
            queue(
                commands,
                state.views[0],
                vec![TextEdit::SelectAll, TextEdit::Backspace],
            );
            state.stage = 14;
            state.since = frames;
        }
        14 if frames - state.since >= 12 => {
            assert_eq!(
                editors.get(state.views[1]).unwrap().value().to_string(),
                "8"
            );
            assert_eq!(
                *prompts.get(state.prompts[0]).unwrap().2,
                Visibility::Inherited
            );
            assert_eq!(
                *prompts.get(state.prompts[1]).unwrap().2,
                Visibility::Hidden
            );
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_font(owner, None);
                runtime.store_mut().set_object_single_line(owner, true);
                runtime.store_mut().set_object_ubb_enabled(owner, false);
                runtime.store_mut().set_object_html_enabled(owner, false);
                runtime.store_mut().set_object_font_size(owner, 30);
                runtime.store_mut().set_input_restrict(owner, None).unwrap();
                runtime.store_mut().set_input_max_length(owner, 0);
                runtime.store_mut().set_input_password(owner, true);
                runtime.store_mut().set_input_editable(owner, i != 1);
                runtime
                    .store_mut()
                    .set_object_text(owner, ["A中😀B", "<b>Read-only</b>", ""][i]);
                runtime
                    .store_mut()
                    .set_input_prompt(owner, Some("Password hint stays readable"));
                commands.entity(state.labels[i]).insert(Text::new(
                    [
                        "Password / UTF-16 mask / real value retained",
                        "Read-only password / literal HTML concealed",
                        "Empty password / readable prompt",
                    ][i],
                ));
            }
            state.stage = 20;
            state.since = frames;
        }
        20 if frames - state.since >= 16 => {
            for (i, expected) in ["A中😀B", "<b>Read-only</b>", ""].iter().enumerate() {
                assert_eq!(
                    editors.get(state.views[i]).unwrap().value().to_string(),
                    *expected
                );
                assert_eq!(
                    layouts.get(state.views[i]).unwrap().1.glyphs.len(),
                    expected.encode_utf16().count()
                );
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-input-passwords.png"));
            state.stage = 21;
            state.since = frames;
        }
        21 if frames - state.since >= 10 => {
            queue(
                commands,
                state.views[0],
                vec![
                    TextEdit::TextEnd(false),
                    TextEdit::Backspace,
                    TextEdit::ImeSetCompose {
                        value: "预😀".into(),
                        cursor: None,
                    },
                ],
            );
            queue(
                commands,
                state.views[1],
                vec![TextEdit::Insert("REJECTED".into())],
            );
            state.stage = 22;
            state.since = frames;
        }
        22 if frames - state.since >= 10 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "A中😀"
            );
            assert_eq!(layouts.get(state.views[0]).unwrap().1.glyphs.len(), 7);
            assert_eq!(
                editors.get(state.views[1]).unwrap().value().to_string(),
                "<b>Read-only</b>"
            );
            queue(
                commands,
                state.views[0],
                vec![TextEdit::ImeCommit {
                    value: "完成".into(),
                }],
            );
            runtime
                .store_mut()
                .set_input_password(state.owners[2], false);
            runtime
                .store_mut()
                .set_object_text(state.owners[2], "Visible after toggle");
            commands
                .entity(state.labels[0])
                .insert(Text::new("Password / IME committed / real value retained"));
            commands
                .entity(state.labels[2])
                .insert(Text::new("Password disabled / original value rendered"));
            state.stage = 23;
            state.since = frames;
        }
        23 if frames - state.since >= 12 => {
            assert_eq!(
                editors.get(state.views[0]).unwrap().value().to_string(),
                "A中😀完成"
            );
            assert_eq!(layouts.get(state.views[0]).unwrap().1.glyphs.len(), 6);
            assert_eq!(
                editors.get(state.views[2]).unwrap().value().to_string(),
                "Visible after toggle"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-input-passwords-updated.png",
                ));
            state.stage = 24;
            state.since = frames;
        }
        24 if frames - state.since >= 10 => {
            runtime.remove_root(commands, state.root.unwrap());
            for &label in &state.labels {
                commands.entity(label).despawn();
            }
            runtime.store_mut().set_visible(original, true);
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| nodes.get(e).is_err()));
            assert!(state.prompts.iter().all(|&e| nodes.get(e).is_err()));
            state.stage = 8;
            println!(
                "demo smoke: native input constraints, clipboard/IME, ordinary and bitmap prompts, readonly rejection/recovery, custom prompt color/font, empty/preedit/commit visibility, password UTF-16 masking/IME/read-only/toggle and disposal verified"
            );
        }
        _ => {}
    }
}
