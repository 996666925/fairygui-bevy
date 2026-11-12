use super::*;
use crate::movie::MovieClipPlayer;

/// The content image is separate from the GObject node: trimming must not
/// move its pivot, children, layout rectangle or pointer hit surface.
#[derive(Component, Debug)]
pub(crate) struct ContentVisual {
    pub root: Entity,
    pub source_size: Vec2,
    pub trim: Option<(Vec2, Vec2)>,
}

pub(super) fn sprite_geometry(
    sprite: Option<&fairygui_core::package::AtlasSprite>,
) -> (Vec2, Option<(Vec2, Vec2)>) {
    sprite.map_or((Vec2::ZERO, None), |sprite| {
        (
            Vec2::new(sprite.original_size.x, sprite.original_size.y),
            Some((
                Vec2::new(sprite.offset.x, sprite.offset.y),
                Vec2::new(sprite.rect.width, sprite.rect.height),
            )),
        )
    })
}

pub(super) fn spawn_visual(
    entity: &mut bevy::ecs::system::EntityCommands<'_>,
    owner: GObjectId,
    image: ImageNode,
    source_size: Vec2,
    trim: Option<(Vec2, Vec2)>,
    grid: Option<crate::image_fill::ImageGrid>,
) -> Entity {
    let root = entity.id();
    let mut visual = None;
    entity.with_children(|parent| {
        let mut child = parent.spawn((
            ContentVisual {
                root,
                source_size,
                trim,
            },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                ..default()
            },
            image,
            Visibility::Hidden,
            bevy::picking::Pickable::IGNORE,
            appearance::TextTintSource { owner, color: None },
        ));
        if let Some(grid) = grid {
            child.insert(grid);
        }
        visual = Some(child.id());
    });
    visual.unwrap()
}

fn trimmed_rect(
    canvas: fairygui_core::geom::Rect,
    source: Vec2,
    trim: Option<(Vec2, Vec2)>,
) -> fairygui_core::geom::Rect {
    let Some((offset, size)) = trim else {
        return canvas;
    };
    if source.x <= 0.0 || source.y <= 0.0 {
        return fairygui_core::geom::Rect::ZERO;
    }
    let scale = Vec2::new(canvas.width / source.x, canvas.height / source.y);
    fairygui_core::geom::Rect::new(
        canvas.x + offset.x * scale.x,
        canvas.y + offset.y * scale.y,
        size.x * scale.x,
        size.y * scale.y,
    )
}

pub(crate) fn sync_content_layout(
    mut runtime: NonSendMut<GuiRuntime>,
    images: Option<Res<Assets<Image>>>,
    roots: Query<&GObjectRef>,
    players: Query<&MovieClipPlayer>,
    mut visuals: Query<(
        &ContentVisual,
        Option<&crate::image_fill::ImageGrid>,
        &mut ImageNode,
        &mut Node,
        &mut Visibility,
    )>,
) {
    use fairygui_core::geom::{Rect as GuiRect, Size};
    for (visual, grid, mut image, mut node, mut visibility) in &mut visuals {
        let Ok(object) = roots.get(visual.root) else {
            continue;
        };
        let Some(gobject) = runtime.store().object(object.0) else {
            continue;
        };
        let mut source = visual.source_size;
        let mut trim = visual.trim;
        let mut blank = false;
        if let Ok(player) = players.get(visual.root) {
            if let Some((atlas, rect)) = player.render_frame() {
                if let Some(atlas) = atlas
                    && &image.image != atlas
                {
                    image.image = atlas.clone();
                }
                if image.rect != Some(rect) {
                    image.rect = Some(rect);
                }
            }
            if let Some(frame) = player.current() {
                source = Vec2::new(frame.original_size.width, frame.original_size.height);
                trim = Some((
                    Vec2::new(frame.offset.x, frame.offset.y),
                    Vec2::new(frame.rect.width, frame.rect.height),
                ));
            } else {
                blank = true;
            }
        } else if source.x <= 0.0 || source.y <= 0.0 {
            source = images
                .as_ref()
                .and_then(|images| images.get(&image.image))
                .map_or(Vec2::ZERO, |image| image.size().as_vec2());
        }
        let source_size = Size::new(source.x, source.y);
        let viewport = gobject.size();
        let auto_size = matches!(gobject.control(), ControlData::Loader(data) if data.auto_size);
        let canvas = if let ControlData::Loader(loader) = gobject.control() {
            loader.content_rect(source_size, viewport)
        } else {
            GuiRect::new(0.0, 0.0, viewport.width, viewport.height)
        };
        if auto_size {
            runtime.store_mut().set_size(
                object.0,
                if source.x > 0.0 { source.x } else { 50.0 },
                if source.y > 0.0 { source.y } else { 30.0 },
            );
        }
        let rect = trimmed_rect(canvas, source, if grid.is_some() { None } else { trim });
        let next = (
            Val::Px(rect.x),
            Val::Px(rect.y),
            Val::Px(rect.width),
            Val::Px(rect.height),
        );
        if (node.left, node.top, node.width, node.height) != next {
            (node.left, node.top, node.width, node.height) = next;
        }
        let visible = !blank
            && rect.width > 0.0
            && rect.height > 0.0
            && !matches!(
                runtime.loader_load_state(object.0),
                Some(GuiLoaderState::Failed(_))
            );
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::geom::{Point, Rect as GuiRect, Size};
    use fairygui_core::package::{FrameTexture, MovieClipData, MovieFrame, ObjectType};

    fn node_rect(node: &Node) -> (Val, Val, Val, Val) {
        (node.left, node.top, node.width, node.height)
    }

    fn setup(kind: ObjectType) -> (App, GObjectId, Entity, Entity) {
        let mut app = App::new();
        let mut gui = GuiRuntime::default();
        let object = gui.store_mut().create(kind);
        gui.store_mut().set_xy(object, 100.0, 200.0);
        gui.store_mut().set_size(object, 200.0, 100.0);
        let root = app
            .world_mut()
            .spawn((
                GObjectRef(object),
                backend::node_for(gui.store().object(object).unwrap()),
                Visibility::Inherited,
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((
                ContentVisual {
                    root,
                    source_size: Vec2::new(100.0, 50.0),
                    trim: Some((Vec2::new(10.0, 5.0), Vec2::new(40.0, 20.0))),
                },
                Node::default(),
                ImageNode::default(),
                Visibility::Hidden,
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        app.world_mut().entity_mut(root).add_child(visual);
        app.insert_non_send(gui)
            .add_systems(Update, sync_content_layout);
        (app, object, root, visual)
    }

    #[test]
    fn changing_movie_trim_moves_only_the_content_sprite_and_blank_frames_keep_the_hit_node() {
        let (mut app, object, root, visual) = setup(ObjectType::MovieClip);
        let frame = |offset: Point, rect: GuiRect| MovieFrame {
            add_delay: 0,
            sprite_id: None,
            texture: Some(FrameTexture {
                atlas_item: 0,
                rect,
                offset,
                original_size: Size::new(100.0, 50.0),
            }),
        };
        let clip = MovieClipData {
            interval: 100,
            swing: false,
            repeat_delay: 0,
            frames: vec![
                frame(Point::new(10.0, 5.0), GuiRect::new(5.0, 10.0, 40.0, 20.0)),
                MovieFrame {
                    add_delay: 0,
                    sprite_id: None,
                    texture: None,
                },
                frame(Point::new(30.0, 0.0), GuiRect::new(55.0, 10.0, 20.0, 30.0)),
            ],
        };
        app.world_mut()
            .entity_mut(root)
            .insert(MovieClipPlayer::new(&clip, 0, false).unwrap());
        let original = node_rect(app.world().get::<Node>(root).unwrap());
        app.update();
        assert_eq!(
            node_rect(app.world().get::<Node>(visual).unwrap()),
            (Val::Px(20.0), Val::Px(10.0), Val::Px(80.0), Val::Px(40.0))
        );
        app.world_mut()
            .get_mut::<MovieClipPlayer>(root)
            .unwrap()
            .goto(1);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(visual).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
        app.world_mut()
            .get_mut::<MovieClipPlayer>(root)
            .unwrap()
            .goto(2);
        app.update();
        assert_eq!(
            node_rect(app.world().get::<Node>(visual).unwrap()),
            (Val::Px(60.0), Val::Px(0.0), Val::Px(40.0), Val::Px(60.0))
        );
        assert_eq!(node_rect(app.world().get::<Node>(root).unwrap()), original);
        assert!(app.world().get::<ImageNode>(root).is_none());
        assert_eq!(
            app.world().get::<bevy::picking::Pickable>(visual),
            Some(&bevy::picking::Pickable::IGNORE)
        );
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object(object)
                .unwrap()
                .size(),
            Size::new(200.0, 100.0)
        );
    }

    #[test]
    fn loader_alignment_and_fill_apply_to_the_full_canvas_before_trimming() {
        let (mut app, object, root, visual) = setup(ObjectType::Loader);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_size(object, 300.0, 200.0);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_loader_layout(object, 1, 1, 1);
        app.update();
        // Fit a 100x50 canvas into 300x200: canvas 300x150 at (0,25).
        assert_eq!(
            node_rect(app.world().get::<Node>(visual).unwrap()),
            (Val::Px(30.0), Val::Px(40.0), Val::Px(120.0), Val::Px(60.0))
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_loader_shrink_only(object, true);
        app.update();
        assert_eq!(
            node_rect(app.world().get::<Node>(visual).unwrap()),
            (Val::Px(110.0), Val::Px(80.0), Val::Px(40.0), Val::Px(20.0))
        );
        assert_eq!(app.world().get::<ChildOf>(visual).unwrap().parent(), root);
    }

    #[test]
    fn loader_auto_size_updates_the_model_from_original_size_not_trim_size() {
        let (mut app, object, _, visual) = setup(ObjectType::Loader);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_loader_auto_size(object, true);
        app.update();
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object(object)
                .unwrap()
                .size(),
            Size::new(100.0, 50.0)
        );
        assert_eq!(
            node_rect(app.world().get::<Node>(visual).unwrap()),
            (Val::Px(10.0), Val::Px(5.0), Val::Px(40.0), Val::Px(20.0))
        );
        app.world_mut()
            .get_mut::<ContentVisual>(visual)
            .unwrap()
            .source_size = Vec2::new(150.0, 75.0);
        app.update();
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object(object)
                .unwrap()
                .size(),
            Size::new(150.0, 75.0)
        );
    }
}
