use super::*;

fn asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fairygui-demo/assets/ui")
        .join(format!("{name}.fui"))
}
#[test]
fn real_packages_generate_deterministic_parseable_typed_children_pages_and_transitions() {
    let first = Bindings::new()
        .package(asset("Basics"))
        .package(asset("ModalWaiting"))
        .generate()
        .unwrap();
    let reversed = Bindings::new()
        .package(asset("ModalWaiting"))
        .package(asset("Basics"))
        .generate()
        .unwrap();
    assert_eq!(first, reversed);
    syn::parse_file(&first).expect("generated source is valid Rust");
    assert!(first.contains("pub struct DemoWindow"));
    assert!(first.contains("pub n0: ::fairygui_bevy::GuiHandle<super::basics::Button1>"));
    assert!(first.contains("::fairygui_bevy::GuiController<"));
    assert!(first.contains("::fairygui_bevy::GuiTransition"));
    assert!(first.contains("fn id(self) -> &'static str"));
    assert!(!first.contains("GuiRuntime"));
}
#[test]
fn every_demo_package_can_be_scanned_without_instantiating_its_runtime_tree() {
    let folder = asset("Basics").parent().unwrap().to_owned();
    let mut bindings = Bindings::new();
    for path in fs::read_dir(folder).unwrap() {
        let path = path.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "fui") {
            bindings = bindings.package(path);
        }
    }
    let source = bindings.generate().unwrap();
    syn::parse_file(&source).expect("all exported metadata generates valid Rust");
}
#[test]
fn foreign_component_references_require_their_package_and_resolve_to_its_generated_type() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fairygui-core/tests/fixtures");
    let host = fixtures.join("PopupHost.fui");
    let theme = fixtures.join("PopupTheme.fui");
    let error = Bindings::new().package(&theme).generate().unwrap_err();
    assert!(
        error.to_string().contains("add dependency package"),
        "{error}"
    );
    let host_package = FuiPackage::parse("host", &fs::read(&host).unwrap()).unwrap();
    let source = Bindings::new()
        .package(host)
        .package(theme)
        .generate()
        .unwrap();
    syn::parse_file(&source).unwrap();
    let host_module = snake(host_package.name());
    assert!(source.contains(&format!("pub mod {host_module}")));
    assert!(source.contains(&format!("GuiHandle<super::{host_module}::")));
}
#[test]
fn identifiers_handle_keywords_punctuation_digits_unicode_and_collisions() {
    for name in [
        "type",
        "Self",
        "gen",
        "123",
        "",
        "a-b",
        "a b",
        "你好",
        "URLButton",
        "a\"b\n",
        "_",
        "::",
    ] {
        syn::parse_str::<syn::Ident>(&snake(name)).unwrap();
        syn::parse_str::<syn::Ident>(&pascal(name)).unwrap();
    }
    assert_eq!(snake("URLButton"), "url_button");
    assert_eq!(pascal("Demo_Window"), "DemoWindow");
    let mut names = Names::default();
    names.reserve("root");
    assert_eq!(names.take("root"), "root_2");
    assert_eq!(names.take(&snake("a-b")), "a_b");
    assert_eq!(names.take(&snake("a b")), "a_b_2");
}
#[test]
fn invalid_inputs_are_errors_and_never_replace_existing_output() {
    assert!(Bindings::new().generate().is_err());
    assert!(
        Bindings::new()
            .package(asset("missing"))
            .generate()
            .is_err()
    );
    assert!(
        Bindings::new()
            .package(asset("Basics"))
            .package(asset("Basics"))
            .generate()
            .is_err()
    );
    assert!(Bindings::new().generate_to_out_dir("../escape.rs").is_err());
    let directory =
        std::env::temp_dir().join(format!("fairygui-build-test-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("bad.fui");
    let output = directory.join("output.rs");
    fs::write(&input, b"invalid package").unwrap();
    fs::write(&output, "existing source").unwrap();
    assert!(
        Bindings::new()
            .package(&input)
            .generate_to(&output)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), "existing source");
    let bindings = Bindings::new().package(asset("Basics"));
    bindings.generate_to(&output).unwrap();
    let timestamp = fs::metadata(&output).unwrap().modified().unwrap();
    bindings.generate_to(&output).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().modified().unwrap(),
        timestamp
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
    fs::remove_dir(directory).unwrap();
}
