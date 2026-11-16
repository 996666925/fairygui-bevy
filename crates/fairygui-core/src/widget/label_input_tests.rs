use super::*;
use crate::{
    object::{GObjectId, ObjectStore},
    widget::ControlData,
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

fn fixture(mode: u8, password: bool) -> (ObjectStore, GObjectId, Option<GObjectId>, GObjectId) {
    let mut store = ObjectStore::new();
    let label = store.create(ObjectType::Label);
    let icon = store.create(ObjectType::Loader);
    store.object_mut(icon).unwrap().name = "icon".into();
    store.set_parent(icon, Some(label));
    store.apply_icon_to(label, "base-icon");
    let leaf = if mode == 4 {
        None
    } else {
        Some(store.create(if mode == 3 {
            ObjectType::Text
        } else {
            ObjectType::InputText
        }))
    };
    if let Some(leaf) = leaf {
        let parent = if matches!(mode, 1 | 2) {
            let parent = store.create(if mode == 1 {
                ObjectType::Label
            } else {
                ObjectType::Button
            });
            store.object_mut(parent).unwrap().name = "title".into();
            store.set_parent(parent, Some(label));
            parent
        } else {
            label
        };
        store.object_mut(leaf).unwrap().name = "title".into();
        store.set_parent(leaf, Some(parent));
        store.set_object_text(label, "Base");
        store.set_control_color(label, Color::from_rgba8(0x12, 0x34, 0x56, 255), None);
        store.set_object_font_size(label, 16);
        if let ControlData::TextInput(data) = &mut store.object_mut(leaf).unwrap().control {
            data.prompt_text = Some("BaseHint".into());
            data.restrict = Some("A-Z".into());
            data.max_length = 8;
            data.keyboard_type = 3;
            data.password = password;
        }
    }
    store.take_dirty();
    (store, label, leaf, icon)
}

#[test]
fn label_instance_text_and_input_overrides_match_original_forwarding_and_preservation() {
    let mut store = ObjectStore::new();
    let mut label = store.create(ObjectType::Label);
    let mut leaf = None;
    let mut icon = label;
    let mut name = "";
    let mut end = 0;
    let mut cases = 0;
    let mut table = StringTable::new();
    for s in ["Changed", "icon-url", "Hint", "0-9"] {
        table.push(Some(s.into()));
    }
    for line in include_str!("../../tests/fixtures/label-input-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let kind = parts.next().unwrap();
        match kind {
            "case" => {
                name = parts.next().unwrap();
                let mode = parts.next().unwrap().parse().unwrap();
                let password = parts.next().unwrap() == "1";
                (store, label, leaf, icon) = fixture(mode, password);
                cases += 1;
            }
            "bytes" => {
                let bytes = decode(parts.next().unwrap());
                let mut buffer = ByteBuffer::new(&bytes);
                setup_label(&mut store, label, &mut buffer, &table, 0, ObjectType::Label).unwrap();
                end = buffer.position();
            }
            "text" => assert_eq!(
                leaf.and_then(|id| store.object_text(id)).unwrap_or(""),
                text(parts.next().unwrap()),
                "{name}: text"
            ),
            "icon" => {
                let ControlData::Loader(data) = store.object(icon).unwrap().control() else {
                    panic!()
                };
                assert_eq!(
                    data.url.as_deref().unwrap(),
                    text(parts.next().unwrap()),
                    "{name}: icon"
                );
            }
            "color" => {
                let rgb =
                    u32::from_str_radix(parts.next().unwrap().trim_start_matches('#'), 16).unwrap();
                assert_eq!(
                    leaf.and_then(|id| store.object_color(id))
                        .unwrap_or(Color::from_rgba8(0, 0, 0, 255)),
                    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255),
                    "{name}: color"
                );
            }
            "font" => assert_eq!(
                leaf.and_then(|id| store.object_text_field(id))
                    .map_or(0, |d| d.font_size),
                parts.next().unwrap().parse().unwrap(),
                "{name}: font"
            ),
            "input" => {
                let ControlData::TextInput(data) = store.object(leaf.unwrap()).unwrap().control()
                else {
                    panic!()
                };
                assert_eq!(
                    data.prompt_text.as_deref(),
                    Some(text(parts.next().unwrap()).as_str()),
                    "{name}: prompt"
                );
                assert_eq!(
                    data.restrict.as_deref(),
                    Some(text(parts.next().unwrap()).as_str()),
                    "{name}: restrict"
                );
                assert_eq!(
                    data.max_length,
                    parts.next().unwrap().parse().unwrap(),
                    "{name}: max"
                );
                assert_eq!(
                    if data.password {
                        -1
                    } else {
                        data.keyboard_type
                    },
                    parts.next().unwrap().parse().unwrap(),
                    "{name}: keyboard"
                );
                assert_eq!(
                    data.password,
                    parts.next().unwrap() == "1",
                    "{name}: password"
                );
            }
            "position" => assert_eq!(
                end,
                parts.next().unwrap().parse::<usize>().unwrap(),
                "{name}: record consumption"
            ),
            _ => panic!("unknown record {kind}"),
        }
    }
    assert_eq!(cases, 14);
}
