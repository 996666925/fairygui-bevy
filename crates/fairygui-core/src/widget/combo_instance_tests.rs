use super::*;
use crate::{
    object::ObjectStore,
    package::ObjectType,
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
struct Scene {
    store: ObjectStore,
    combo: GObjectId,
    leaf: Option<GObjectId>,
    icon: GObjectId,
}
fn fixture(nested: u8, preload: bool, binding: bool) -> Scene {
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
            ..Default::default()
        });
    let combo = store.create(ObjectType::ComboBox);
    store.set_parent(combo, Some(parent));
    let leaf = if nested == 3 {
        None
    } else {
        let holder = if nested == 0 {
            combo
        } else {
            let holder = store.create(if nested == 1 {
                ObjectType::Label
            } else {
                ObjectType::Button
            });
            store.object_mut(holder).unwrap().name = "title".into();
            store.set_parent(holder, Some(combo));
            holder
        };
        let leaf = store.create(ObjectType::Text);
        store.object_mut(leaf).unwrap().name = "title".into();
        store.set_parent(leaf, Some(holder));
        Some(leaf)
    };
    let icon = store.create(ObjectType::Loader);
    store.object_mut(icon).unwrap().name = "icon".into();
    store.set_parent(icon, Some(combo));
    store.set_object_text(combo, "Base");
    store.apply_icon_to(combo, "base-icon");
    store.set_control_color(combo, Color::from_rgba8(18, 52, 86, 255), None);
    store.set_object_font_size(combo, 16);
    if binding {
        store.set_combo_selection_controller(combo, Some("pages"));
    }
    if preload {
        let ControlData::ComboBox(data) = &mut store.object_mut(combo).unwrap().control else {
            unreachable!()
        };
        data.items = ["First", "Changed", "First"].map(str::to_owned).to_vec();
        data.values = ["v1", "v2", "v1"].map(str::to_owned).to_vec();
        data.icons = Some(vec![Some("item-icon".into()); 2]);
    }
    Scene {
        store,
        combo,
        leaf,
        icon,
    }
}

#[test]
fn combo_instance_properties_match_original_forwarding_null_preservation_and_item_updates() {
    let mut table = StringTable::new();
    for value in [
        "First",
        "Second",
        "Changed",
        "v1",
        "v2",
        "icon-url",
        "item-icon",
    ] {
        table.push(Some(value.into()));
    }
    let mut scene = fixture(0, false, false);
    let mut name = "";
    let mut end = 0;
    let mut cases = 0;
    for line in include_str!("../../tests/fixtures/combo-instance-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut p = line.split_whitespace();
        let kind = p.next().unwrap();
        match kind {
            "case" => {
                name = p.next().unwrap();
                scene = fixture(
                    p.next().unwrap().parse().unwrap(),
                    p.next().unwrap() == "1",
                    p.next().unwrap() == "1",
                );
                cases += 1;
            }
            "bytes" => {
                let bytes = decode(p.next().unwrap());
                let mut buffer = ByteBuffer::new(&bytes);
                read_combo(&mut scene.store, scene.combo, &mut buffer, &table, 0).unwrap();
                end = buffer.position();
            }
            "text" => assert_eq!(
                scene
                    .leaf
                    .and_then(|id| scene.store.object_text(id))
                    .unwrap_or_default(),
                text(p.next().unwrap()),
                "{name}: title"
            ),
            "icon" => {
                let ControlData::Loader(data) = scene.store.object(scene.icon).unwrap().control()
                else {
                    unreachable!()
                };
                assert_eq!(
                    data.url.as_deref().unwrap_or_default(),
                    text(p.next().unwrap()),
                    "{name}: icon"
                );
            }
            "color" => {
                let rgb =
                    u32::from_str_radix(p.next().unwrap().trim_start_matches('#'), 16).unwrap();
                assert_eq!(
                    scene
                        .leaf
                        .and_then(|id| scene.store.object_color(id))
                        .unwrap_or(Color::from_rgba8(0, 0, 0, 255)),
                    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255),
                    "{name}: color"
                );
            }
            "position" => assert_eq!(
                end,
                p.next().unwrap().parse::<usize>().unwrap(),
                "{name}: consumed bytes"
            ),
            _ => {
                let ControlData::ComboBox(data) =
                    scene.store.object(scene.combo).unwrap().control()
                else {
                    unreachable!()
                };
                match kind {
                    "selection" => assert_eq!(
                        data.selected_index,
                        p.next().unwrap().parse().unwrap(),
                        "{name}: selection"
                    ),
                    "settings" => {
                        assert_eq!(
                            data.visible_item_count,
                            p.next().unwrap().parse().unwrap(),
                            "{name}: visible count"
                        );
                        assert_eq!(
                            data.popup_direction,
                            p.next().unwrap().parse().unwrap(),
                            "{name}: direction"
                        );
                        assert_eq!(
                            scene
                                .store
                                .combo_selection_controller(scene.combo)
                                .is_some(),
                            p.next().unwrap() == "1",
                            "{name}: controller"
                        );
                    }
                    "items" | "values" => {
                        let list: Vec<_> = p.collect();
                        let expected: Vec<_> = if list == ["-"] {
                            Vec::new()
                        } else {
                            list.into_iter().map(text).collect()
                        };
                        assert_eq!(
                            if kind == "items" {
                                &data.items
                            } else {
                                &data.values
                            },
                            &expected,
                            "{name}: {kind}"
                        );
                    }
                    "icons" => {
                        let list: Vec<_> = p.collect();
                        let expected = if list == ["none"] {
                            None
                        } else {
                            Some(
                                list.into_iter()
                                    .map(|s| if s == "null" { None } else { Some(text(s)) })
                                    .collect(),
                            )
                        };
                        assert_eq!(data.icons, expected, "{name}: sparse icons");
                    }
                    _ => panic!("unexpected {kind}"),
                }
            }
        }
    }
    assert_eq!(cases, 18);
}
