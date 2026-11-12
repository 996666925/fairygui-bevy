use super::*;
use crate::movie::MovieClipPlayer;

#[derive(Component, Debug)]
pub(crate) struct MovieSource {
    pub url: String,
    pub visual: Option<Entity>,
}

pub(super) fn player_for(
    asset: &FuiAsset,
    asset_server: &AssetServer,
    index: usize,
    frame: i32,
    playing: bool,
) -> Option<MovieClipPlayer> {
    let clip = asset
        .package
        .load_movie_clip(index, &fairygui_core::package::LoadContext::none())
        .ok()?;
    let mut player = MovieClipPlayer::new(&clip, frame, playing)?;
    for frame in &clip.frames {
        if let Some(texture) = &frame.texture
            && let Some(file) = asset
                .package
                .item(texture.atlas_item)
                .and_then(|atlas| atlas.file.as_deref())
        {
            player.bind_atlas(
                texture.atlas_item,
                asset_server.load(asset.resolve_asset_path(file)),
            );
        }
    }
    Some(player)
}

pub(crate) fn sync_movie_sources(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &GObjectRef, Option<&MovieSource>)>,
) {
    for (entity, object, current) in &query {
        let Some(gobject) = runtime.store().object(object.0) else {
            continue;
        };
        let ControlData::Loader(data) = gobject.control() else {
            continue;
        };
        let data = data.clone();
        let url = data.url.as_deref().unwrap_or("");
        if !runtime.loader_retries.contains(&object.0)
            && current.is_some_and(|current| current.url == url)
        {
            continue;
        }
        if let Some(visual) = current.and_then(|current| current.visual) {
            commands.entity(visual).despawn();
        }
        let mut entity = commands.entity(entity);
        entity.remove::<(MovieClipPlayer, ImageNode)>();
        if url.is_empty() {
            entity.insert(MovieSource {
                url: String::new(),
                visual: None,
            });
            if data.auto_size {
                runtime.store_mut().set_size(object.0, 50.0, 30.0);
            }
            continue;
        }
        let package = if url.starts_with("ui://") {
            assets.iter().find_map(|(_, asset)| {
                asset
                    .package
                    .resolve_local_url(url)
                    .map(|index| (asset, Some(index)))
            })
        } else {
            runtime
                .asset_handle_for(object.0)
                .and_then(|handle| assets.get(&handle))
                .map(|asset| (asset, None))
        };
        let Some((asset, index)) = package else {
            entity.insert(MovieSource {
                url: String::new(),
                visual: None,
            });
            if data.auto_size {
                runtime.store_mut().set_size(object.0, 50.0, 30.0);
            }
            continue;
        };
        if index.is_some_and(|index| {
            asset.package.item(index).is_some_and(|item| {
                item.item_type == fairygui_core::package::PackageItemType::Component
            })
        }) {
            // Components have a model-owned subtree, synchronized before views.
            // Their color/Playing/Frame properties do not target that subtree.
            entity.insert(MovieSource {
                url: url.to_string(),
                visual: None,
            });
            continue;
        }
        let mut image = ImageNode {
            image_mode: bevy::ui::widget::NodeImageMode::Stretch,
            ..default()
        };
        let mut grid = None;
        let (source_size, trim) = if let Some(player) = index
            .and_then(|index| player_for(asset, &asset_server, index, data.frame, data.playing))
        {
            player.apply_frame(&mut image);
            entity.insert(player);
            (
                index
                    .and_then(|index| asset.package.item(index))
                    .map_or(Vec2::ZERO, |item| Vec2::new(item.width, item.height)),
                None,
            )
        } else if let Some(source) = resolve_loader_url(asset, url) {
            image.image = asset_server.load(source.image);
            image.rect = source.sprite.as_ref().map(|sprite| {
                Rect::new(
                    sprite.rect.x,
                    sprite.rect.y,
                    sprite.rect.x + sprite.rect.width,
                    sprite.rect.y + sprite.rect.height,
                )
            });
            grid = source.grid;
            if let Some(sprite) = &source.sprite {
                image = backend::image_for(
                    sprite,
                    image.image,
                    grid.map(|g| g.grid),
                    source.scale_by_tile,
                );
            }
            content::sprite_geometry(source.sprite.as_ref())
        } else {
            entity.insert(MovieSource {
                url: String::new(),
                visual: None,
            });
            if data.auto_size {
                runtime.store_mut().set_size(object.0, 50.0, 30.0);
            }
            continue;
        };
        let visual = content::spawn_visual(&mut entity, object.0, image, source_size, trim, grid);
        entity.insert((
            MovieSource {
                url: url.to_string(),
                visual: Some(visual),
            },
            if runtime.store().display_visible(object.0) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::package::{LoadContext, ObjectType, PackageItemType};

    #[test]
    fn loader_switches_between_exported_movie_and_image_urls_without_replacing_its_entity() {
        let package = FuiPackage::parse(
            "Bag",
            include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui"),
        )
        .unwrap();
        let (movie_index, clip) = package
            .items()
            .iter()
            .enumerate()
            .find_map(|(index, item)| {
                if item.item_type != PackageItemType::MovieClip {
                    return None;
                }
                let clip = package.load_movie_clip(index, &LoadContext::none()).ok()?;
                (clip.frame_count() > 1).then_some((index, clip))
            })
            .expect("bundled movie");
        let image_index = package
            .items()
            .iter()
            .enumerate()
            .find(|(index, item)| {
                item.item_type == PackageItemType::Image && package.sprite_for(*index).is_some()
            })
            .unwrap()
            .0;
        let movie_url = format!(
            "ui://{}{}",
            package.id(),
            package.item(movie_index).unwrap().id
        );
        let image_url = format!(
            "ui://{}{}",
            package.id(),
            package.item(image_index).unwrap().id
        );
        let expected = package.sprite_for(image_index).unwrap().rect;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<FuiAsset>()
            .init_asset::<Image>();
        let mut gui = GuiRuntime::default();
        let loader = gui.store_mut().create(ObjectType::Loader);
        gui.store_mut().apply_icon_to(loader, &image_url);
        app.insert_non_send(gui)
            .add_systems(Update, sync_movie_sources);
        let _package_handle = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package,
                res_key: "ui/Bag".into(),
            });
        let entity = app
            .world_mut()
            .spawn((GObjectRef(loader), Visibility::Inherited))
            .id();
        app.update();
        let visual = app.world().get::<Children>(entity).unwrap()[0];
        assert!(app.world().get::<ImageNode>(visual).is_some());
        assert!(app.world().get::<MovieClipPlayer>(entity).is_none());
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().apply_icon_to(loader, &movie_url);
            gui.store_mut().set_animation_frame(loader, 1);
            gui.store_mut().set_animation_playing(loader, false);
        }
        app.update();
        let player = app.world().get::<MovieClipPlayer>(entity).unwrap();
        assert_eq!(player.frame_count(), clip.frame_count());
        assert_eq!(player.frame(), 1);
        assert!(!player.is_playing());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .apply_icon_to(loader, &image_url);
        app.update();
        assert!(app.world().get::<MovieClipPlayer>(entity).is_none());
        let visual = app.world().get::<Children>(entity).unwrap()[0];
        assert_eq!(
            app.world().get::<ImageNode>(visual).unwrap().rect,
            Some(Rect::new(
                expected.x,
                expected.y,
                expected.x + expected.width,
                expected.y + expected.height
            ))
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .apply_icon_to(loader, "");
        app.update();
        assert!(app.world().get::<ImageNode>(entity).is_none());
        assert!(app.world().get_entity(visual).is_err());
        assert!(app.world().get_entity(entity).is_ok());
    }
}
