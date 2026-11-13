//! ECS queries for FairyGUI roots and logical control hierarchies.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use fairygui_core::package::ObjectType;

use crate::runtime::{GObjectRef, GuiRootInstance, GuiRuntime};

/// Rendered root entity belonging to a [`crate::FairyGuiRoot`] owner.
/// Available after `GuiSystems::Sync`; removed when the root is replaced.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiRootView(pub Entity);

/// Read-only identity and logical hierarchy of a rendered FairyGUI control.
/// Decorative Bevy entities are excluded, and children retain FairyGUI order.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiObjectInfo {
    name: String,
    kind: ObjectType,
    parent: Option<Entity>,
    children: Vec<Entity>,
}

impl GuiObjectInfo {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> ObjectType {
        self.kind
    }

    pub fn parent(&self) -> Option<Entity> {
        self.parent
    }

    pub fn children(&self) -> &[Entity] {
        &self.children
    }
}

/// Read FairyGUI identities and find control entities using only ECS queries.
///
/// Every method accepts either a root owner or a control view entity. In
/// `GuiSystems::Respond` this reads the preceding Sync snapshot; systems after
/// `GuiSystems::Sync` can bind observers to controls created this frame.
#[derive(SystemParam, Debug)]
pub struct GuiQuery<'w, 's> {
    roots: Query<'w, 's, &'static GuiRootView>,
    objects: Query<'w, 's, &'static GuiObjectInfo>,
}

impl GuiQuery<'_, '_> {
    /// Resolve a root owner to its view; control entities resolve to themselves.
    pub fn view(&self, entity: Entity) -> Option<Entity> {
        if self.objects.contains(entity) {
            return Some(entity);
        }
        let view = self.roots.get(entity).ok()?.0;
        self.objects.contains(view).then_some(view)
    }

    pub fn get(&self, entity: Entity) -> Option<&GuiObjectInfo> {
        self.objects.get(self.view(entity)?).ok()
    }

    /// First direct child with the exact FairyGUI object name.
    pub fn child_by_name(&self, parent: Entity, name: &str) -> Option<Entity> {
        self.get(parent)?
            .children
            .iter()
            .copied()
            .find(|&child| self.objects.get(child).is_ok_and(|info| info.name == name))
    }

    /// Find a descendant using FairyGUI's direct-children-before-subtrees order.
    /// The root itself is excluded, as in `ObjectStore::find_by_name`.
    pub fn find_by_name(&self, root: Entity, name: &str) -> Option<Entity> {
        if let Some(child) = self.child_by_name(root, name) {
            return Some(child);
        }
        for &child in &self.get(root)?.children {
            if let Some(found) = self.find_by_name(child, name) {
                return Some(found);
            }
        }
        None
    }

    /// Follow exact child names separated by `/`. An empty path returns the view.
    pub fn child_by_path(&self, root: Entity, path: &str) -> Option<Entity> {
        let mut entity = self.view(root)?;
        if !path.is_empty() {
            for name in path.split('/') {
                entity = self.child_by_name(entity, name)?;
            }
        }
        Some(entity)
    }
}

pub(crate) fn sync_gui_hierarchy(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut objects: Query<(Entity, &GObjectRef, Option<&mut GuiObjectInfo>)>,
    roots: Query<(Entity, &GuiRootInstance, Option<&GuiRootView>)>,
) {
    for (entity, reference, current) in &mut objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(object) = runtime.store().object(reference.0) else {
            continue;
        };
        let parent = object.parent().and_then(|id| runtime.entity_of(id));
        let children = object
            .children()
            .iter()
            .filter_map(|&id| runtime.entity_of(id));
        if let Some(mut current) = current {
            if current.name != object.name() {
                current.name = object.name().to_owned();
            }
            if current.kind != object.kind() {
                current.kind = object.kind();
            }
            if current.parent != parent {
                current.parent = parent;
            }
            if !current.children.iter().copied().eq(children.clone()) {
                current.children = children.collect();
            }
        } else {
            commands.entity(entity).insert(GuiObjectInfo {
                name: object.name().to_owned(),
                kind: object.kind(),
                parent,
                children: children.collect(),
            });
        }
    }

    for (owner, instance, current) in &roots {
        match runtime.entity_of(instance.object) {
            Some(view) if current.map(|current| current.0) != Some(view) => {
                commands.entity(owner).insert(GuiRootView(view));
            }
            None if current.is_some() => {
                commands.entity(owner).remove::<GuiRootView>();
            }
            _ => {}
        }
    }
}
