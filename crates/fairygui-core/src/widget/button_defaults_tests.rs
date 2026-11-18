use super::*;
use crate::{object::ObjectStore, widget::ControlData};

fn sound(value: &str) -> Option<String> {
    match value {
        "-" => None,
        "~" => Some(String::new()),
        value => Some(value.into()),
    }
}
fn index(value: &str, table: &mut StringTable) -> [u8; 2] {
    let index = match value {
        "-" => 65534,
        "~" => 65533,
        value => {
            let index = table.len() as u16;
            table.push(Some(value.into()));
            index
        }
    };
    index.to_be_bytes()
}
fn block(data: Vec<u8>) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes[0] = 7;
    bytes[1] = 1;
    bytes[14..16].copy_from_slice(&16u16.to_be_bytes());
    bytes.extend(data);
    bytes
}
fn button(store: &ObjectStore, id: crate::object::GObjectId) -> &ButtonData {
    let ControlData::Button(button) = store.object(id).unwrap().control() else {
        panic!()
    };
    button
}

#[test]
fn defaults_resource_inheritance_and_instance_muting_match_288_native_cases() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/button-defaults-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let (request, expected) = line.split_once(" | ").unwrap();
        let p: Vec<_> = request.split_whitespace().collect();
        let q: Vec<_> = expected.split_whitespace().collect();
        let mut store = ObjectStore::new();
        store.set_button_defaults(ButtonDefaults {
            sound: sound(p[0]),
            sound_volume_scale: p[1].parse().unwrap(),
        });
        let id = store.create(ObjectType::Button);
        assert_eq!(button(&store, id).sound, sound(q[0]), "{line}");
        assert_eq!(
            button(&store, id).sound_volume_scale,
            q[1].parse::<f32>().unwrap(),
            "{line}"
        );
        let mut table = StringTable::new();
        if p[2] == "1" {
            let mut resource = vec![0];
            resource.extend(index(p[3], &mut table));
            resource.extend(p[4].parse::<f32>().unwrap().to_be_bytes());
            resource.push(0);
            resource.extend(0.8f32.to_be_bytes());
            let bytes = block(resource);
            let ControlData::Button(data) = &mut store.object_mut(id).unwrap().control else {
                panic!()
            };
            setup_button_extension(data, &mut ByteBuffer::new(&bytes), &table).unwrap();
        }
        let mut instance = vec![12];
        for _ in 0..4 {
            instance.extend(65534u16.to_be_bytes());
        }
        instance.push(0);
        instance.extend(0i32.to_be_bytes());
        instance.extend((-1i16).to_be_bytes());
        instance.extend(65534u16.to_be_bytes());
        instance.extend(index(p[5], &mut table));
        instance.push(u8::from(p[6] != "-"));
        if p[6] != "-" {
            instance.extend(p[6].parse::<f32>().unwrap().to_be_bytes());
        }
        instance.push(0);
        let bytes = block(instance);
        setup_button(
            &mut store,
            id,
            &mut ByteBuffer::new(&bytes),
            &table,
            0,
            ObjectType::Button,
        )
        .unwrap();
        assert_eq!(button(&store, id).sound, sound(q[2]), "{line}");
        assert_eq!(
            button(&store, id).sound_volume_scale,
            q[3].parse::<f32>().unwrap(),
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 288);
}

#[test]
fn changed_defaults_only_affect_new_objects_and_nonfinite_volume_keeps_last_default() {
    let mut store = ObjectStore::new();
    assert_eq!(store.button_defaults(), &ButtonDefaults::default());
    store.set_button_defaults(ButtonDefaults {
        sound: Some("first.wav".into()),
        sound_volume_scale: 2.,
    });
    let first = store.create(ObjectType::Button);
    store.set_button_defaults(ButtonDefaults {
        sound: Some("next.wav".into()),
        sound_volume_scale: f32::NAN,
    });
    let next = store.create(ObjectType::Button);
    assert_eq!(button(&store, first).sound.as_deref(), Some("first.wav"));
    assert_eq!(button(&store, next).sound.as_deref(), Some("next.wav"));
    assert_eq!(button(&store, next).sound_volume_scale, 2.);
    // A missing extension leaves constructor defaults intact.
    let ControlData::Button(data) = &mut store.object_mut(next).unwrap().control else {
        panic!()
    };
    setup_button_extension(data, &mut ByteBuffer::new(&[0; 16]), &StringTable::new()).unwrap();
    assert_eq!(button(&store, next).sound_volume_scale, 2.);
}

#[test]
fn real_packages_keep_defaults_through_sync_and_postorder_construction() {
    let package = crate::FuiPackage::parse(
        "ButtonDefaults",
        include_bytes!("../../tests/fixtures/ButtonDefaults.fui"),
    )
    .unwrap();
    for name in ["Button1", "Button2"] {
        let mut store = ObjectStore::new();
        store.set_button_defaults(ButtonDefaults {
            sound: Some("global.wav".into()),
            sound_volume_scale: 0.5,
        });
        let index = package.item_index_by_name(name).unwrap();
        let root = crate::widget::create_from_package(
            &mut store,
            &package,
            index,
            &crate::package::LoadContext::none(),
        )
        .unwrap();
        assert_eq!(button(&store, root).sound.as_deref(), Some("global.wav"));
        let mut builder = crate::async_build::AsyncBuilder::new([index], 1);
        while !builder.advance(&mut store, &package, &crate::package::LoadContext::none()) {}
        assert_eq!(
            button(&store, builder.roots()[0]).sound.as_deref(),
            Some("global.wav")
        );
        assert_eq!(
            button(&store, builder.roots()[0]).sound_volume_scale,
            button(&store, root).sound_volume_scale
        );
    }
}
