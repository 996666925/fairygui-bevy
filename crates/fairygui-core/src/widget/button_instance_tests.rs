use super::*;
use crate::{
    object::{GObjectId, ObjectStore},
    widget::{ControlData, Controller},
};

fn decode(hex: &str) -> Vec<u8> {
    if hex == "-" {
        return Vec::new();
    }
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn text(hex: &str) -> String {
    String::from_utf8(decode(hex)).unwrap()
}

fn fixture(
    mode: i8,
    nested: u8,
    selected: bool,
    building: bool,
) -> (
    ObjectStore,
    GObjectId,
    GObjectId,
    Option<GObjectId>,
    GObjectId,
) {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    store
        .object_mut(parent)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "pages".into(),
            page_ids: vec!["first-id".into(), "second-id".into()],
            page_names: vec!["first".into(), "second".into()],
            selected_index: 0,
            auto_radio_group_depth: true,
            ..Default::default()
        });
    store.object_mut(parent).unwrap().under_construct = building;
    let button = store.create(ObjectType::Button);
    store.set_parent(button, Some(parent));
    let peer = store.create(ObjectType::Button);
    store.set_parent(peer, Some(parent));
    store.set_button_related_controller(peer, Some("pages"));
    let leaf = if nested == 3 {
        None
    } else {
        let holder = if nested == 0 {
            button
        } else {
            let holder = store.create(if nested == 1 {
                ObjectType::Label
            } else {
                ObjectType::Button
            });
            store.object_mut(holder).unwrap().name = "title".into();
            store.set_parent(holder, Some(button));
            holder
        };
        let leaf = store.create(ObjectType::Text);
        store.object_mut(leaf).unwrap().name = "title".into();
        store.set_parent(leaf, Some(holder));
        Some(leaf)
    };
    let icon = store.create(ObjectType::Loader);
    store.object_mut(icon).unwrap().name = "icon".into();
    store.set_parent(icon, Some(button));
    store.set_object_text(button, "Base");
    store.apply_icon_to(button, "base-icon");
    store.set_button_selected_title(button, Some("BaseChosen"));
    store.set_button_selected_icon(button, Some("base-selected"));
    store.set_control_color(button, Color::from_rgba8(0x12, 0x34, 0x56, 255), None);
    store.set_object_font_size(button, 16);
    store.set_button_sound(button, Some("base-sound"));
    store.set_button_sound_volume(button, 0.5);
    let ControlData::Button(data) = &mut store.object_mut(button).unwrap().control else {
        unreachable!()
    };
    data.mode = ButtonMode::from_code(mode);
    data.selected = selected;
    store.initialize_button(button);
    (store, parent, button, leaf, icon)
}

#[test]
fn button_instance_properties_match_original_setter_forwarding_and_selection() {
    let mut table = StringTable::new();
    for value in [
        "Changed",
        "Chosen",
        "icon-url",
        "selected-url",
        "new-sound",
        "second-id",
    ] {
        table.push(Some(value.into()));
    }
    let (mut store, mut parent, mut button, mut leaf, mut icon) = fixture(0, 0, false, true);
    let mut name = "";
    let mut end = 0;
    let mut cases = 0;
    for line in include_str!("../../tests/fixtures/button-instance-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let kind = parts.next().unwrap();
        match kind {
            "case" => {
                name = parts.next().unwrap();
                let mode = parts.next().unwrap().parse().unwrap();
                let nested = parts.next().unwrap().parse().unwrap();
                let initial = parts.next().unwrap() == "1";
                let building = parts.next().unwrap() == "1";
                (store, parent, button, leaf, icon) = fixture(mode, nested, initial, building);
                if parts.next().unwrap() == "1" {
                    store.set_button_related_controller(button, Some("pages"));
                }
                cases += 1;
            }
            "bytes" => {
                let bytes = decode(parts.next().unwrap());
                let mut buffer = ByteBuffer::new(&bytes);
                setup_button(
                    &mut store,
                    button,
                    &mut buffer,
                    &table,
                    0,
                    ObjectType::Button,
                )
                .unwrap();
                end = buffer.position();
            }
            "text" => {
                assert_eq!(
                    store.object_text(button).unwrap(),
                    text(parts.next().unwrap()),
                    "{name}: base title"
                );
                assert_eq!(
                    leaf.and_then(|id| store.object_text(id))
                        .unwrap_or_default(),
                    text(parts.next().unwrap()),
                    "{name}: visible title"
                );
            }
            "icons" => {
                let ControlData::Button(data) = store.object(button).unwrap().control() else {
                    unreachable!()
                };
                assert_eq!(
                    data.icon.as_deref().unwrap_or_default(),
                    text(parts.next().unwrap()),
                    "{name}: base icon"
                );
                assert_eq!(
                    data.selected_icon.as_deref().unwrap_or_default(),
                    text(parts.next().unwrap()),
                    "{name}: selected icon"
                );
                let ControlData::Loader(data) = store.object(icon).unwrap().control() else {
                    unreachable!()
                };
                assert_eq!(
                    data.url.as_deref().unwrap_or_default(),
                    text(parts.next().unwrap()),
                    "{name}: visible icon"
                );
            }
            "style" => {
                let rgb =
                    u32::from_str_radix(parts.next().unwrap().trim_start_matches('#'), 16).unwrap();
                assert_eq!(
                    leaf.and_then(|id| store.object_color(id))
                        .unwrap_or(Color::from_rgba8(0, 0, 0, 255)),
                    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255),
                    "{name}: color"
                );
                assert_eq!(
                    leaf.and_then(|id| store.object_text_field(id))
                        .map_or(0, |d| d.font_size),
                    parts.next().unwrap().parse().unwrap(),
                    "{name}: font"
                );
            }
            "selected" => assert_eq!(
                store.button_selected(button),
                Some(parts.next().unwrap() == "1"),
                "{name}: selected"
            ),
            "binding" => assert_eq!(
                store.button_related_controller(button).is_some(),
                parts.next().unwrap() == "1",
                "{name}: related controller"
            ),
            "sound" => {
                let ControlData::Button(data) = store.object(button).unwrap().control() else {
                    unreachable!()
                };
                assert_eq!(
                    data.sound.as_deref().unwrap_or_default(),
                    text(parts.next().unwrap()),
                    "{name}: sound"
                );
                assert_eq!(
                    data.sound_volume_scale,
                    parts.next().unwrap().parse::<f32>().unwrap(),
                    "{name}: volume"
                );
            }
            "parent" => {
                let controller = store
                    .object(parent)
                    .unwrap()
                    .control()
                    .component()
                    .unwrap()
                    .controller("pages")
                    .unwrap();
                assert_eq!(
                    controller.selected_page_id().unwrap(),
                    text(parts.next().unwrap()),
                    "{name}: parent page"
                );
                let depth: usize = parts.next().unwrap().parse().unwrap();
                assert_eq!(
                    store.child_index(button),
                    Some(depth),
                    "{name}: construction must preserve display order"
                );
            }
            "position" => assert_eq!(
                end,
                parts.next().unwrap().parse::<usize>().unwrap(),
                "{name}: bytes consumed"
            ),
            _ => panic!("unknown record {kind}"),
        }
    }
    assert_eq!(cases, 18);
}
