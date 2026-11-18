use super::*;
use crate::{
    async_build::AsyncBuilder,
    object::{GObjectId, ObjectStore},
    package::{FuiPackage, LoadContext, PackageRegistry},
    widget::{create_from_package, create_from_package_with_registry},
};
use std::sync::Arc;

fn text(value: &str) -> String {
    if value == "~" {
        String::new()
    } else {
        value.into()
    }
}
fn string_index(value: &str, table: &mut StringTable) -> [u8; 2] {
    let index = table.len() as u16;
    table.push(Some(value.into()));
    index.to_be_bytes()
}

#[test]
fn homepage_versions_branches_missing_and_empty_variables_match_2352_native_cases() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/controller-home-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let (request, expected) = line.split_once(" | ").unwrap();
        let p: Vec<_> = request.split_whitespace().collect();
        let q: Vec<_> = expected.split_whitespace().collect();
        let mut table = StringTable::new();
        let mut name = Vec::from(string_index("env", &mut table));
        name.push(0);
        let pages = p[5].parse::<u16>().unwrap();
        let mut page_data = pages.to_be_bytes().to_vec();
        for i in 0..pages as usize {
            page_data.extend(string_index(&format!("id{i}"), &mut table));
            page_data.extend(string_index(&text(p[6 + i]), &mut table));
        }
        let kind: u8 = p[1].parse().unwrap();
        page_data.push(kind);
        if kind == 1 {
            page_data.extend(p[2].parse::<i16>().unwrap().to_be_bytes());
        }
        if kind == 3 {
            page_data.extend(string_index("mode", &mut table));
        }
        let mut bytes = vec![3, 1];
        let mut offset = 8u16;
        for size in [name.len(), page_data.len(), 2] {
            bytes.extend(offset.to_be_bytes());
            offset += size as u16;
        }
        bytes.extend(name);
        bytes.extend(page_data);
        bytes.extend(0u16.to_be_bytes());
        let mut buffer = ByteBuffer::new(&bytes);
        buffer.set_version(p[0].parse().unwrap());
        let mut env = PackageEnv {
            branch: text(p[3]),
            ..Default::default()
        };
        if p[4] != "-" {
            env.vars.insert("mode".into(), text(p[4]));
        }
        let mut controller = Controller::default();
        controller.setup(&mut buffer, &table, &env).unwrap();
        assert_eq!(
            controller.selected_index,
            q[0].parse::<i32>().unwrap(),
            "{line}"
        );
        assert_eq!(
            controller.previous_index,
            q[1].parse::<i32>().unwrap(),
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 2352);
}

fn fixture() -> (Arc<FuiPackage>, PackageRegistry) {
    let package = Arc::new(
        FuiPackage::parse(
            "ControllerEnv",
            include_bytes!("../../tests/fixtures/ControllerEnv.fui"),
        )
        .unwrap(),
    );
    let mut registry = PackageRegistry::new();
    registry.add(package.clone(), "ui/ControllerEnv");
    (package, registry)
}
fn page(store: &ObjectStore, owner: GObjectId, name: &str) -> i32 {
    store
        .object(owner)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller(name)
        .unwrap()
        .selected_index
}
fn build(
    store: &mut ObjectStore,
    package: &FuiPackage,
    registry: &PackageRegistry,
    name: &str,
) -> GObjectId {
    create_from_package_with_registry(
        store,
        package,
        package.item_index_by_name(name).unwrap(),
        &registry.load_context(package),
        registry,
    )
    .unwrap()
}

#[test]
fn real_component_home_rules_use_global_environment_before_initial_gears() {
    let (package, mut registry) = fixture();
    let mut store = ObjectStore::new();
    let first = build(&mut store, &package, &registry, "Demo_Controller");
    assert_eq!(
        (page(&store, first, "c1"), page(&store, first, "c2")),
        (0, 0)
    );
    registry.set_var("mode", "");
    let second = build(&mut store, &package, &registry, "Demo_Controller");
    assert_eq!(page(&store, second, "c1"), 1);
    store.set_controller_page_index(first, "c1", 1);
    store.advance_gear_tweens(10.);
    for name in ["n3", "n6", "n8", "n9"] {
        let a = store.child_by_name(first, name).unwrap();
        let b = store.child_by_name(second, name).unwrap();
        assert_eq!(
            store.object(a).unwrap().position(),
            store.object(b).unwrap().position(),
            "{name}"
        );
        assert_eq!(store.display_visible(a), store.display_visible(b), "{name}");
    }
    registry.set_branch("night");
    assert_eq!(registry.load_context(&package).branch_index, -1);
    let third = build(&mut store, &package, &registry, "Demo_Controller");
    assert_eq!(
        page(&store, third, "c2"),
        2,
        "home pages use the global name, not the local asset branch index"
    );
    registry.set_vars(HashMap::new());
    assert_eq!(registry.get_var("mode"), None);
    assert!(registry.vars().is_empty());
    let fourth = build(&mut store, &package, &registry, "Demo_Controller");
    assert_eq!(page(&store, fourth, "c1"), 0);
    assert_eq!(
        page(&store, second, "c1"),
        1,
        "changing globals does not reselect an existing controller"
    );
}

#[test]
fn standalone_no_branch_does_not_activate_the_first_asset_branch() {
    let (package, _) = fixture();
    let mut store = ObjectStore::new();
    let index = package.item_index_by_name("Demo_Controller").unwrap();
    let root = create_from_package(&mut store, &package, index, &LoadContext::none()).unwrap();
    assert_eq!(page(&store, root, "c2"), 0);
    let root = create_from_package(
        &mut store,
        &package,
        index,
        &LoadContext {
            branch_index: 0,
            ..LoadContext::none()
        },
    )
    .unwrap();
    assert_eq!(page(&store, root, "c2"), 1);
}

#[test]
fn nested_and_pending_components_read_live_variables_at_construction() {
    let (package, mut registry) = fixture();
    let mut store = ObjectStore::new();
    let index = package.item_index_by_name("Demo_Popup").unwrap();
    let context = registry.load_context(&package);
    let mut builder = AsyncBuilder::new([index], 1);
    assert!(!builder.advance_with_registry(&mut store, &package, &context, &registry));
    registry.set_var("mode", "");
    registry.set_branch("night");
    while !builder.advance_with_registry(&mut store, &package, &context, &registry) {}
    assert_eq!(builder.failures(), 0);
    let parent = builder.roots()[0];
    let nested = store.object(parent).unwrap().children()[0];
    assert_eq!(
        (page(&store, nested, "c1"), page(&store, nested, "c2")),
        (1, 2)
    );
}
