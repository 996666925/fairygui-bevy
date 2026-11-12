use super::*;

impl GuiRuntime {
    /// Public removal queues can run before the normal sync system. Repair
    /// surviving logical objects first so recursive Bevy despawn cannot reclaim
    /// a child detached/reparented earlier in this same application frame.
    pub(super) fn preserve_surviving_views(&self, commands: &mut Commands) {
        let mut images_to_preserve = Vec::new();
        for (&object, &entity) in &self.entities {
            let Some(gobject) = self.store.object(object) else {
                continue;
            };
            let parent = gobject
                .parent()
                .and_then(|id| self.entities.get(&id).copied());
            images_to_preserve.push((entity, parent));
            if let Some(parent) = parent {
                commands.entity(entity).insert(ChildOf(parent));
            } else {
                commands.entity(entity).remove::<ChildOf>();
            }
        }
        // Read the relationship target when commands apply, so parts spawned
        // earlier in this frame are included too. This runs before any despawn.
        commands.queue(move |world: &mut World| {
            for (owner, parent) in images_to_preserve {
                let images: Vec<_> = world
                    .get::<InlineImageParts>(owner)
                    .map(|parts| parts.iter().collect())
                    .unwrap_or_default();
                for image in images {
                    if let Ok(mut entity) = world.get_entity_mut(image) {
                        if let Some(parent) = parent {
                            entity.insert(ChildOf(parent));
                        } else {
                            entity.remove::<ChildOf>();
                        }
                    }
                }
            }
        });
    }
}

/// Mirror logical parenting and stage state without destroying pooled/detached
/// entities. Relationships are updated before removed parents are despawned.
type StageView = (
    Entity,
    &'static GObjectRef,
    Option<&'static ChildOf>,
    &'static mut Visibility,
    Option<&'static GlobalZIndex>,
    Has<ScrollBarOverlay>,
);

pub(crate) fn sync_stage_hierarchy(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut query: Query<StageView>,
) {
    let mut removed = Vec::new();
    for (entity, object, parent, mut visibility, global_z, was_overlay) in &mut query {
        if runtime.entity_of(object.0) != Some(entity) {
            continue;
        }
        let Some(gobject) = runtime.store().object(object.0) else {
            if parent.is_some() {
                commands.entity(entity).remove::<ChildOf>();
            }
            removed.push((object.0, entity));
            continue;
        };
        let overlay = runtime.store().is_scroll_overlay(object.0);
        if overlay && !was_overlay {
            commands.entity(entity).insert((
                ScrollBarOverlay,
                bevy::ui::IgnoreScroll(bevy::math::BVec2::TRUE),
            ));
        } else if !overlay && was_overlay {
            commands
                .entity(entity)
                .remove::<(ScrollBarOverlay, bevy::ui::IgnoreScroll)>();
        }
        // Standalone pages can be created after GRoot. Keep its window/popup
        // subtree above ordinary roots for both rendering and UI picking.
        // An explicit application override remains authoritative.
        if runtime.root_container() == Some(object.0) && global_z.is_none() {
            commands.entity(entity).insert(GlobalZIndex(1));
        }
        let logical_parent = gobject.parent();
        let next = logical_parent.and_then(|id| runtime.entity_of(id));
        if parent.map(ChildOf::parent) != next {
            if let Some(parent) = next {
                commands.entity(entity).insert(ChildOf(parent));
            } else {
                commands.entity(entity).remove::<ChildOf>();
            }
        }
        let visible = runtime.store().is_on_stage(object.0)
            && runtime.store().display_visible(object.0)
            && (logical_parent.is_none() || next.is_some());
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (object, entity) in removed {
        runtime.entities.remove(&object);
        runtime.pending_spawn.retain(|id| *id != object);
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::package::ObjectType;

    #[test]
    fn detached_views_survive_parent_disposal_and_reconnect_without_replacing_entities() {
        let mut gui = GuiRuntime::default();
        let parent = gui.store_mut().create(ObjectType::Component);
        let child = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(child, Some(parent));
        gui.store_mut().attach_to_stage(parent);
        let mut app = App::new();
        app.insert_non_send(gui)
            .add_systems(Update, sync_stage_hierarchy);
        let parent_entity = app
            .world_mut()
            .spawn((GObjectRef(parent), Visibility::Inherited))
            .id();
        let child_entity = app
            .world_mut()
            .spawn((
                GObjectRef(child),
                Visibility::Inherited,
                ChildOf(parent_entity),
            ))
            .id();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.entities.insert(parent, parent_entity);
            gui.entities.insert(child, child_entity);
        }
        app.update();
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .detach_from_parent(child);
        // The detached entity still has its old Bevy parent when the logical
        // parent is deleted. Synchronization must detach it before despawning.
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .remove(parent);
        app.update();
        assert!(app.world().get_entity(parent_entity).is_err());
        assert!(app.world().get_entity(child_entity).is_ok());
        assert!(app.world().get::<ChildOf>(child_entity).is_none());
        assert_eq!(
            app.world().get::<Visibility>(child_entity),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().non_send::<GuiRuntime>().entity_of(parent), None);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .attach_to_stage(child);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(child_entity),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().non_send::<GuiRuntime>().entity_of(child),
            Some(child_entity)
        );
    }

    #[test]
    fn logical_reparent_updates_childof_and_visibility_without_changing_explicit_visible() {
        let mut gui = GuiRuntime::default();
        let first = gui.store_mut().create(ObjectType::Component);
        let second = gui.store_mut().create(ObjectType::Component);
        let child = gui.store_mut().create(ObjectType::Graph);
        gui.store_mut().attach_to_stage(first);
        gui.store_mut().attach_to_stage(second);
        gui.store_mut().set_parent(child, Some(first));
        let mut app = App::new();
        app.insert_non_send(gui)
            .add_systems(Update, sync_stage_hierarchy);
        let a = app
            .world_mut()
            .spawn((GObjectRef(first), Visibility::Inherited))
            .id();
        let b = app
            .world_mut()
            .spawn((GObjectRef(second), Visibility::Inherited))
            .id();
        let c = app
            .world_mut()
            .spawn((GObjectRef(child), Visibility::Inherited, ChildOf(a)))
            .id();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.entities.extend([(first, a), (second, b), (child, c)]);
            gui.store_mut().set_parent(child, Some(second));
        }
        app.update();
        assert_eq!(app.world().get::<ChildOf>(c).unwrap().parent(), b);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .detach_from_stage(second);
        app.update();
        assert_eq!(app.world().get::<Visibility>(c), Some(&Visibility::Hidden));
        assert!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object(child)
                .unwrap()
                .visible()
        );
    }

    #[test]
    fn root_attachment_handles_pending_views_and_restarts_auto_play_only_on_real_reentry() {
        let package = FuiPackage::parse(
            "Transition",
            include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
        )
        .unwrap();
        let mut gui = GuiRuntime::default();
        let index = gui
            .create_root(Handle::default(), &package, "PowerUp")
            .unwrap();
        let owner = gui.root_object(index).unwrap();
        gui.store_mut()
            .set_transition_auto_play(owner, "t0", true)
            .unwrap();
        assert!(!gui.store().is_on_stage(owner));
        gui.set_root_attached(index, false);
        gui.roots[index].spawned = true; // model a completed view build
        gui.set_root_attached(index, true);
        assert!(gui.store().is_on_stage(owner));
        gui.store_mut().advance_transitions(0.25);
        gui.set_root_attached(index, true);
        assert_eq!(gui.store().transition_state(owner, "t0").unwrap().1, 0.25);
        gui.set_root_attached(index, false);
        assert!(!gui.store().transition_state(owner, "t0").unwrap().0);
        gui.set_root_attached(index, true);
        assert_eq!(gui.store().transition_state(owner, "t0").unwrap().1, 0.0);
        let parent = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().attach_to_stage(parent);
        gui.store_mut().set_parent(owner, Some(parent));
        assert!(gui.set_root_attached(index, false));
        assert!(!gui.store().is_on_stage(owner));
        assert_eq!(gui.store().object(owner).unwrap().parent(), None);
        assert!(gui.set_root_attached(index, true));
        gui.store_mut().remove(owner);
        assert_eq!(gui.root_object(index), None);
        assert!(!gui.set_root_attached(index, true));
    }

    #[test]
    fn hidden_runtime_windows_keep_their_subtrees_and_close_removes_all_entity_mappings() {
        let package = FuiPackage::parse(
            "MainMenu",
            include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
        )
        .unwrap();
        let mut gui = GuiRuntime::default();
        gui.create_root(Handle::default(), &package, "Main")
            .unwrap();
        let index = gui
            .show_window(
                &package,
                "Main",
                false,
                fairygui_core::geom::Size::new(1136.0, 640.0),
            )
            .unwrap();
        let owner = gui.window(index).unwrap().owner();
        let mut objects = vec![owner];
        gui.store()
            .for_each_descendant(owner, &mut |id| objects.push(id));
        assert!(objects.len() > 1);
        let mut world = World::new();
        let entities: Vec<_> = objects
            .iter()
            .map(|id| {
                let entity = world.spawn_empty().id();
                gui.entities.insert(*id, entity);
                entity
            })
            .collect();
        assert!(gui.hide_window(index));
        assert!(gui.hide_window(index));
        assert!(objects.iter().all(|id| gui.store().contains(*id)));
        assert!(!gui.store().is_on_stage(owner));
        assert!(gui.show_existing_window(index));
        assert!(gui.show_existing_window(index));
        assert!(gui.store().is_on_stage(owner));
        assert_eq!(gui.entity_of(owner), Some(entities[0]));
        let hide_events = gui
            .drain_events()
            .filter(|event| {
                event.kind == fairygui_core::object::EventKind::WindowHide && event.is_at_target()
            })
            .count();
        assert_eq!(hide_events, 1);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        gui.close_window(&mut Commands::new(&mut queue, &world), index);
        queue.apply(&mut world);
        assert!(
            objects
                .iter()
                .all(|id| !gui.store().contains(*id) && gui.entity_of(*id).is_none())
        );
        assert!(entities.iter().all(|id| world.get_entity(*id).is_err()));
    }

    #[test]
    fn removing_a_root_in_the_same_frame_as_logical_detach_preserves_the_pooled_entity() {
        let package = FuiPackage::parse(
            "MainMenu",
            include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
        )
        .unwrap();
        let mut gui = GuiRuntime::default();
        let index = gui
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        let child = gui.store().child_at(root, 0).unwrap();
        let mut world = World::new();
        let parent_entity = world.spawn_empty().id();
        let child_entity = world.spawn(ChildOf(parent_entity)).id();
        gui.entities
            .extend([(root, parent_entity), (child, child_entity)]);
        gui.pool.release(&mut gui.store, "ui://pooled", child);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        gui.remove_root(&mut Commands::new(&mut queue, &world), index);
        queue.apply(&mut world);
        assert!(world.get_entity(parent_entity).is_err());
        assert!(world.get_entity(child_entity).is_ok());
        assert!(world.get::<ChildOf>(child_entity).is_none());
        assert!(gui.store().contains(child));
        assert_eq!(gui.entity_of(child), Some(child_entity));
        let acquired = gui.pool.acquire_checked(&gui.store, "ui://pooled").unwrap();
        assert_eq!(acquired, child);
        gui.store.attach_to_stage(acquired);
        assert!(gui.store.is_on_stage(child));
        assert_eq!(gui.entity_of(child), Some(child_entity));
    }
}
