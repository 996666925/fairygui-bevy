//! ECS root instances mounted in the logical FairyGUI hierarchy.
use super::*;
use std::collections::HashSet;

#[cfg(test)]
#[path = "ecs_root_mount_tests.rs"]
mod tests;

/// Mount result on the FairyGuiRoot owner. Mounted instances can still inherit
/// an off-stage or hidden parent. Unchanged results keep their change tick.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiRootMountState {
    WaitingForAsset,
    WaitingForParent,
    Mounted { parent: Option<Entity> },
    Detached,
    InvalidParent,
    ParentCycle,
    CreationFailed,
}

fn resolve(
    runtime: &GuiRuntime,
    references: &Query<&GObjectRef>,
    entity: Entity,
) -> Option<GObjectId> {
    runtime
        .ecs_root_object(entity)
        .or_else(|| runtime.ecs_window_object(entity))
        .or_else(|| references.get(entity).ok().map(|reference| reference.0))
        .filter(|&object| runtime.store.contains(object))
}

fn cycle(
    runtime: &GuiRuntime,
    owners: &HashMap<GObjectId, Entity>,
    sources: &Query<(Entity, &FairyGuiRoot, Option<&GuiRootMountState>)>,
    references: &Query<&GObjectRef>,
    owner: Entity,
) -> bool {
    let mut visited = HashSet::new();
    let mut current = Some(owner);
    while let Some(entity) = current {
        if !visited.insert(entity) {
            return true;
        }
        let Ok((_, source, _)) = sources.get(entity) else {
            return false;
        };
        let Some(parent) = source.parent else {
            return false;
        };
        if sources.contains(parent) {
            current = Some(parent);
        } else {
            let mut object = resolve(runtime, references, parent);
            current = None;
            while let Some(id) = object {
                if let Some(&owner) = owners.get(&id) {
                    current = Some(owner);
                    break;
                }
                object = runtime.store.object(id).and_then(|o| o.parent());
            }
        }
    }
    false
}

/// Detach explicit moves before a removed/replaced parent recursively reclaims
/// its old subtree. Native/model and Bevy survival then follow the same intent.
pub(crate) fn prepare_mounts(
    sources: Query<(Entity, &FairyGuiRoot)>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for (owner, source) in &sources {
        let Some(binding) = runtime.ecs_roots.get(&owner) else {
            continue;
        };
        if (binding.parent != source.parent || !source.active)
            && let Some(object) = binding
                .handle
                .and_then(|handle| runtime.root_object(handle))
            && binding.parent.is_some()
        {
            runtime.store.detach_from_parent(object);
            runtime.store.detach_from_stage(object);
        }
    }
}

pub(crate) fn sync_mounts(
    mut commands: Commands,
    sources: Query<(Entity, &FairyGuiRoot, Option<&GuiRootMountState>)>,
    references: Query<&GObjectRef>,
    entities: Query<()>,
    window_sources: Query<&windows::FairyGuiWindow>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    // Root models are created first, so owner targets resolve independently of
    // entity/query order, even for multiple newly spawned nesting levels.
    let owners: HashMap<_, _> = runtime
        .ecs_roots
        .iter()
        .filter_map(|(&owner, binding)| {
            binding
                .handle
                .and_then(|handle| runtime.root_object(handle))
                .map(|object| (object, owner))
        })
        .collect();
    let states: Vec<_> = sources
        .iter()
        .map(|(owner, source, current)| {
            let Some(binding) = runtime.ecs_roots.get(&owner) else {
                return (owner, current.copied(), GuiRootMountState::WaitingForAsset);
            };
            let Some((handle, object)) = binding
                .handle
                .and_then(|handle| runtime.root_object(handle).map(|object| (handle, object)))
            else {
                return (
                    owner,
                    current.copied(),
                    if binding.failed {
                        GuiRootMountState::CreationFailed
                    } else {
                        GuiRootMountState::WaitingForAsset
                    },
                );
            };
            let was_nested = binding.parent.is_some();
            let parent = source
                .parent
                .and_then(|parent| resolve(&runtime, &references, parent));
            let status = if cycle(&runtime, &owners, &sources, &references, owner) {
                GuiRootMountState::ParentCycle
            } else if source.parent.is_some() && parent.is_none() {
                let target = source.parent.unwrap();
                if entities.contains(target)
                    && !sources.contains(target)
                    && !window_sources.contains(target)
                {
                    GuiRootMountState::InvalidParent
                } else {
                    GuiRootMountState::WaitingForParent
                }
            } else if parent.is_some_and(|parent| {
                runtime
                    .store
                    .object(parent)
                    .is_none_or(|o| o.control().component().is_none())
            }) {
                GuiRootMountState::InvalidParent
            } else if !source.active {
                GuiRootMountState::Detached
            } else {
                GuiRootMountState::Mounted {
                    parent: source.parent,
                }
            };

            if source.parent.is_some() || was_nested {
                runtime.roots[handle].attached = false;
                runtime.store.detach_from_stage(object);
                let mounted_parent = matches!(status, GuiRootMountState::Mounted { .. })
                    .then_some(parent)
                    .flatten();
                // set_parent rejects descendant cycles. The declared graph above
                // rejects owner cycles before either side can form a partial mount.
                runtime.store.set_parent(object, mounted_parent);
                if source.parent.is_none()
                    && source.active
                    && matches!(status, GuiRootMountState::Mounted { .. })
                {
                    runtime.roots[handle].attached = true;
                    if runtime.roots[handle].spawned {
                        runtime.store.attach_to_stage(object);
                    }
                }
            }
            runtime.ecs_roots.get_mut(&owner).unwrap().parent = source.parent;
            (owner, current.copied(), status)
        })
        .collect();
    for (owner, current, next) in states {
        if current != Some(next) {
            commands.entity(owner).insert(next);
        }
    }
}

pub(super) fn waiting_for_parent(runtime: &GuiRuntime, object: GObjectId) -> bool {
    runtime.ecs_roots.values().any(|binding| {
        binding.parent.is_some()
            && binding
                .handle
                .and_then(|handle| runtime.root_object(handle))
                == Some(object)
            && runtime
                .store
                .object(object)
                .is_some_and(|object| object.parent().is_none())
    })
}
