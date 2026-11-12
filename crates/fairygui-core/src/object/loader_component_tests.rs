use super::*;

#[test]
fn unmodified_native_loader_component_layouts_match_fill_alignment_shrink_auto_and_resize_modes() {
    let mut store = ObjectStore::new();
    let (mut loader, mut component, mut source) = (None, None, Size::ZERO);
    for line in include_str!("../../tests/fixtures/loader-component-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(case) = line.strip_prefix("case ") {
            let fields: Vec<_> = case.split_whitespace().collect();
            source = Size::new(fields[1].parse().unwrap(), fields[2].parse().unwrap());
            store = ObjectStore::new();
            let owner = store.create(ObjectType::Loader);
            let content = store.create(ObjectType::Component);
            store.set_size(content, source.width, source.height);
            store.set_parent(content, Some(owner));
            loader = Some(owner);
            component = Some(content);
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap();
        let fields: Vec<_> = command.split_whitespace().collect();
        let loader = loader.unwrap();
        let component = component.unwrap();
        store.set_loader_layout(
            loader,
            fields[1].parse().unwrap(),
            fields[3].parse().unwrap(),
            fields[4].parse().unwrap(),
        );
        store.set_loader_shrink_only(loader, fields[2] == "1");
        store.set_loader_use_resize(loader, fields[5] == "1");
        store.set_loader_auto_size(loader, fields[6] == "1");
        store.set_size(
            loader,
            fields[7].parse().unwrap(),
            fields[8].parse().unwrap(),
        );
        assert!(store.layout_loader_component(loader, component, source));
        let owner = store.object(loader).unwrap();
        let content = store.object(component).unwrap();
        let actual = [
            owner.width(),
            owner.height(),
            content.x(),
            content.y(),
            content.width(),
            content.height(),
            content.scale_x(),
            content.scale_y(),
        ];
        for (actual, expected) in actual.into_iter().zip(expected.split_whitespace()) {
            let expected: f32 = expected.parse().unwrap();
            assert!(
                (actual - expected).abs() < 0.0001,
                "{line}: {actual} != {expected}"
            );
        }
    }
}
