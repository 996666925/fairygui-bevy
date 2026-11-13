use super::*;
use fairygui_core::{geom::Size, package::ObjectType};

fn fixture() -> (App, FuiPackage, GObjectId) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .register_asset_loader(bevy::image::ImageLoader::new(
            bevy::image::CompressedImageFormats::NONE,
        ))
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "correct/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let root = gui.create_root(handle, &package, "Demo_Loader").unwrap();
    let root = gui.root_object(root).unwrap();
    let loader = gui.store.child_by_name(root, "n2").unwrap();
    gui.store.set_loader_show_error_sign(loader, true);
    gui.store.apply_icon_to(loader, "ui://Missing/Resource");
    gui.set_loader_error_sign(Some("ui://Basics/PopupMenuItem"));
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_movie_sources,
            sync_loader_components,
            sync_loader_status,
            spawn_pending_roots,
            spawn_pending_objects,
            content::sync_content_layout,
            sync_dirty,
            sync_stage_hierarchy,
            loader_status::sync_loader_states,
        )
            .chain(),
    );
    (app, package, loader)
}

#[test]
fn loader_state_component_tracks_failure_pending_ready_empty_and_keeps_unchanged_ticks() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, _, loader) = fixture();
    app.update();
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(loader)
        .unwrap();
    assert_eq!(
        app.world().get::<GuiLoaderState>(view),
        Some(&GuiLoaderState::Failed(GuiLoaderFailure::MissingResource))
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiLoaderState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiLoaderState>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "ui://Basics/PopupMenuItem");
    app.world_mut()
        .run_system_once(loader_status::sync_loader_states)
        .unwrap();
    assert_eq!(
        app.world().get::<GuiLoaderState>(view),
        Some(&GuiLoaderState::Loading),
        "URL changes must not expose the previous failure"
    );
    app.update();
    assert_eq!(
        app.world().get::<GuiLoaderState>(view),
        Some(&GuiLoaderState::Ready)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "");
    app.update();
    assert_eq!(
        app.world().get::<GuiLoaderState>(view),
        Some(&GuiLoaderState::Empty)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .remove(loader);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn first_failure_creates_a_sized_error_sign_and_clear_reuses_its_retained_model_and_entity() {
    let (mut app, _, loader) = fixture();
    app.update();
    let (sign, view, size) = {
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(
            gui.loader_load_state(loader),
            Some(GuiLoaderState::Failed(GuiLoaderFailure::MissingResource))
        );
        let sign = gui.loader_error_sign(loader).unwrap();
        let size = gui.store.object(loader).unwrap().size();
        assert_eq!(gui.store.object(sign).unwrap().size(), size);
        assert_eq!(gui.store.object(sign).unwrap().parent(), Some(loader));
        (sign, gui.entity_of(sign).unwrap(), size)
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .set_size(loader, 333.0, 222.0);
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store
            .object(sign)
            .unwrap()
            .size(),
        size,
        "native error size is captured on failure, not on every layout"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "");
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none()
    );
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "ui://Missing/Again");
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.loader_error_sign(loader), Some(sign));
    assert_eq!(gui.entity_of(sign), Some(view));
    assert_eq!(
        gui.store.object(sign).unwrap().size(),
        Size::new(333.0, 222.0)
    );
}

#[test]
fn show_error_flag_and_configuration_changes_affect_new_failures_and_explicit_retry() {
    let (mut app, _, loader) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .set_loader_show_error_sign(loader, false);
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .set_loader_show_error_sign(loader, true);
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none(),
        "flag setter alone does not call setErrorState"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    let sign = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_error_sign(loader)
        .unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store.set_loader_show_error_sign(loader, false);
        gui.set_loader_error_sign(Some("ui://Basics/Component12"));
    }
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader),
        Some(sign)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none()
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store.set_loader_show_error_sign(loader, true);
        gui.retry_loader(loader);
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let replacement = gui.loader_error_sign(loader).unwrap();
    assert_ne!(replacement, sign);
    assert_eq!(
        gui.store.object(replacement).unwrap().package_item_id(),
        gui.registry
            .get_by_name("Basics")
            .unwrap()
            .item_by_name("Component12")
            .map(|item| item.id.as_str())
    );
}

#[test]
fn error_pool_is_fifo_preserves_state_and_accepts_canonical_resource_aliases() {
    let (mut app, package, first) = fixture();
    let second = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let second = gui.store.create(ObjectType::Loader);
        let parent = gui.store.object(first).unwrap().parent();
        gui.store.set_parent(second, parent);
        gui.store.apply_icon_to(second, "ui://Missing/Second");
        gui.queue_subtree(second);
        second
    };
    app.update();
    let (a, b) = {
        let gui = app.world().non_send::<GuiRuntime>();
        (
            gui.loader_error_sign(first).unwrap(),
            gui.loader_error_sign(second).unwrap(),
        )
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .set_object_text(a, "Preserved");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(first, "");
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(second, "");
    app.update();
    let resource = package.item_by_name("PopupMenuItem").unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.set_loader_error_sign(Some(&format!("ui://{}{}", package.id(), resource.id)));
        gui.store.apply_icon_to(first, "ui://Missing/A");
        gui.store.apply_icon_to(second, "ui://Missing/B");
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.loader_error_sign(first), Some(a));
    assert_eq!(gui.loader_error_sign(second), Some(b));
    assert!(
        matches!(gui.store.object(a).unwrap().control(), ControlData::Button(data) if data.title == "Preserved")
    );
}

#[test]
fn component_success_recovery_failure_retry_and_owner_disposal_clear_error_ownership() {
    let (mut app, _, loader) = fixture();
    app.update();
    let sign = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_error_sign(loader)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "ui://Basics/Component12");
    app.update();
    let original = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_component(loader)
        .unwrap();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_load_state(loader),
        Some(GuiLoaderState::Ready)
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    let replacement = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_component(loader)
        .unwrap();
    assert_ne!(replacement, original);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store
            .contains(original)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "ui://Missing/Fail");
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader),
        Some(sign)
    );
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(sign)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .remove(loader);
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(gui.loader_load_state(loader).is_none());
    assert!(gui.loader_error_sign(loader).is_none());
    assert!(!gui.store.contains(sign));
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn unsupported_resources_and_missing_error_templates_do_not_create_fake_images_or_partial_objects()
{
    let (mut app, package, loader) = fixture();
    let unsupported = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::Sound)
        .unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.set_loader_error_sign(Some("ui://Missing/Sign"));
        gui.store
            .apply_icon_to(loader, &format!("ui://{}{}", package.id(), unsupported.id));
    }
    app.update();
    let count = app.world().non_send::<GuiRuntime>().store.len();
    for _ in 0..3 {
        app.update();
    }
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.store.len(), count);
    assert_eq!(
        gui.loader_load_state(loader),
        Some(GuiLoaderState::Failed(
            GuiLoaderFailure::UnsupportedResource
        ))
    );
    assert!(gui.loader_error_sign(loader).is_none());
    let entity = gui.entity_of(loader).unwrap();
    assert!(
        app.world()
            .get::<MovieSource>(entity)
            .unwrap()
            .visual
            .is_none()
    );
}

fn wait_for_failure(app: &mut App, loader: GObjectId) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.update();
        if app
            .world()
            .non_send::<GuiRuntime>()
            .loader_load_state(loader)
            == Some(GuiLoaderState::Failed(GuiLoaderFailure::ImageLoad))
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "asset failure timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn external_image_failure_retry_and_success_follow_actual_asset_server_state() {
    let (mut app, _, loader) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "missing-loader-image-for-status-test.png");
    // Query synchronously before Bevy receives its background asset result.
    let path = "correct/missing-loader-image-for-status-test.png";
    let handle: Handle<Image> = app.world().resource::<AssetServer>().load(path);
    assert_eq!(
        image_state(
            app.world().resource::<AssetServer>(),
            Some(app.world().resource::<Assets<Image>>()),
            path.into(),
            false,
        ),
        GuiLoaderState::Loading
    );
    wait_for_failure(&mut app, loader);
    let sign = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_error_sign(loader)
        .unwrap();
    let visual = {
        let gui = app.world().non_send::<GuiRuntime>();
        let view = gui.entity_of(loader).unwrap();
        app.world()
            .get::<MovieSource>(view)
            .unwrap()
            .visual
            .unwrap()
    };
    assert_eq!(
        app.world().get::<Visibility>(visual),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_load_state(loader),
        Some(GuiLoaderState::Loading)
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader)
            .is_none()
    );
    wait_for_failure(&mut app, loader);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader),
        Some(sign)
    );
    // Supply the completed image using the same handle, as AssetServer would.
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(handle.id(), Image::default())
        .unwrap();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.loader_load_state(loader), Some(GuiLoaderState::Ready));
    assert!(gui.loader_error_sign(loader).is_none());
    let visual = app
        .world()
        .get::<MovieSource>(gui.entity_of(loader).unwrap())
        .unwrap()
        .visual
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(visual),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn movie_status_tracks_atlas_failure_and_recovery_and_late_package_registration() {
    let (mut app, _, loader) = fixture();
    let package = FuiPackage::parse(
        "Bag",
        include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui"),
    )
    .unwrap();
    let (index, clip) = package
        .items()
        .iter()
        .enumerate()
        .find_map(|(index, item)| {
            (item.item_type == PackageItemType::MovieClip)
                .then(|| {
                    package
                        .load_movie_clip(index, &fairygui_core::package::LoadContext::none())
                        .ok()
                })
                .flatten()
                .filter(|clip| clip.frames.iter().any(|frame| frame.texture.is_some()))
                .map(|clip| (index, clip))
        })
        .unwrap();
    let url = format!("ui://{}{}", package.id(), package.item(index).unwrap().id);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, &url);
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_load_state(loader),
        Some(GuiLoaderState::Failed(GuiLoaderFailure::MissingResource))
    );
    let sign = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_error_sign(loader)
        .unwrap();
    let asset = FuiAsset {
        package,
        res_key: "missing-status-assets/Bag".into(),
    };
    let paths: std::collections::HashSet<_> = clip
        .frames
        .iter()
        .filter_map(|frame| frame.texture.as_ref())
        .map(|texture| {
            asset.resolve_asset_path(
                asset
                    .package
                    .item(texture.atlas_item)
                    .unwrap()
                    .file
                    .as_deref()
                    .unwrap(),
            )
        })
        .collect();
    let _package_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(asset);
    wait_for_failure(&mut app, loader);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(loader),
        Some(sign)
    );
    for path in paths {
        let handle: Handle<Image> = app.world().resource::<AssetServer>().load(path);
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(handle.id(), Image::default())
            .unwrap();
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.loader_load_state(loader), Some(GuiLoaderState::Ready));
    assert!(gui.loader_error_sign(loader).is_none());
    assert!(
        app.world()
            .get::<crate::movie::MovieClipPlayer>(gui.entity_of(loader).unwrap())
            .is_some()
    );
}

#[test]
fn recursive_error_templates_and_destroyed_pool_entries_stay_bounded() {
    let (mut app, _, loader) = fixture();
    let theme = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/PopupTheme.fui"),
    )
    .unwrap();
    let _handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: theme,
            res_key: "theme/Basics".into(),
        });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_loader_error_sign(Some("ui://PopupTheme/Demo_Loader"));
    app.update();
    let sign = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_error_sign(loader)
        .unwrap();
    let nested = app
        .world()
        .non_send::<GuiRuntime>()
        .store
        .child_by_name(sign, "n1")
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(nested, "ui://Missing/Nested");
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_error_sign(nested)
            .is_none()
    );
    let count = app.world().non_send::<GuiRuntime>().store.len();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().non_send::<GuiRuntime>().store.len(), count);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(loader, "");
    app.update();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store.remove(sign);
        gui.store.apply_icon_to(loader, "ui://Missing/Again");
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let replacement = gui.loader_error_sign(loader).unwrap();
    assert_ne!(replacement, sign);
    assert!(gui.store.contains(replacement));
}

#[test]
fn native_error_lifecycle_trace_matches_size_flags_configuration_and_fifo_state() {
    let (mut app, package, first) = fixture();
    let second = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let loaders: Vec<_> = gui
            .store
            .iter()
            .filter_map(|(id, object)| {
                matches!(object.control(), ControlData::Loader(_)).then_some(id)
            })
            .collect();
        for id in loaders {
            gui.store.apply_icon_to(id, "");
            gui.store.set_loader_show_error_sign(id, id == first);
        }
        gui.store.set_size(first, 0.0, 0.0);
        gui.set_loader_error_sign(None);
        let second = gui.store.create(ObjectType::Loader);
        let parent = gui.store.object(first).unwrap().parent();
        gui.store.set_parent(second, parent);
        gui.store.set_size(second, 0.0, 0.0);
        gui.queue_subtree(second);
        second
    };
    let loaders = [first, second];
    let mut identities = std::collections::HashMap::new();
    let alias = format!(
        "ui://{}{}",
        package.id(),
        package.item_by_name("PopupMenuItem").unwrap().id
    );
    for (step, line) in
        include_str!("../../fairygui-core/tests/fixtures/loader-error-reference.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .enumerate()
    {
        let (command, expected) = line.split_once(" | ").unwrap();
        let parts: Vec<_> = command.split_whitespace().collect();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            if parts[0] == "config" {
                gui.set_loader_error_sign(match parts[1] {
                    "A" => Some("ui://Basics/PopupMenuItem"),
                    "B" => Some("ui://Basics/Component12"),
                    "alias" => Some(&alias),
                    "none" => None,
                    _ => unreachable!(),
                });
            } else {
                let loader = loaders[parts[1].parse::<usize>().unwrap()];
                match parts[0] {
                    "size" => {
                        gui.store.set_size(
                            loader,
                            parts[2].parse().unwrap(),
                            parts[3].parse().unwrap(),
                        );
                    }
                    "show" => {
                        gui.store
                            .set_loader_show_error_sign(loader, parts[2] == "1");
                    }
                    "fail" => {
                        gui.store
                            .apply_icon_to(loader, &format!("ui://Missing/Step{step}"));
                    }
                    "retry" => {
                        assert!(gui.retry_loader(loader));
                    }
                    "clear" => {
                        gui.store.apply_icon_to(loader, "");
                    }
                    "text" => {
                        let sign = gui.loader_error_sign(loader).unwrap();
                        gui.store.set_object_text(sign, parts[2]);
                    }
                    _ => unreachable!(),
                }
            }
        }
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let mut snapshot = Vec::new();
        for loader in loaders {
            if let Some(sign) = gui.loader_error_sign(loader) {
                let next = identities.len() + 1;
                let id = identities.entry(sign).or_insert(next);
                let object = gui.store.object(sign).unwrap();
                let title = if matches!(object.control(), ControlData::Button(data) if data.title == "preserved")
                {
                    "preserved"
                } else {
                    "-"
                };
                snapshot.extend([
                    id.to_string(),
                    object.width().to_string(),
                    object.height().to_string(),
                    title.into(),
                    u8::from(object.parent() == Some(loader)).to_string(),
                ]);
            } else {
                snapshot.extend(["0", "0", "0", "-", "0"].map(str::to_owned));
            }
        }
        snapshot.push(
            gui.loader_error_pool
                .values()
                .map(|pool| pool.len())
                .sum::<usize>()
                .to_string(),
        );
        assert_eq!(snapshot.join(" "), expected, "step {step}: {command}");
    }
}
