//! Package extensions attach application components to retained control views.
use bevy::{ecs::system::EntityCommands, prelude::*};
use std::collections::HashMap;

use crate::{GObjectRef, GuiQuery, GuiRuntime};

/// A component constructed once for a matching exported package component.
/// Register before creating its views. Constructors see the complete logical
/// subtree and snapshots; pooled/detached views retain the same component.
pub trait GuiExtension: Component + Sized {
    const URL: &'static str;
    fn construct(entity: Entity, gui: &GuiQuery) -> Result<Self, GuiExtensionError>;
    /// Attach observers or issue entity-addressed commands after inserting Self.
    fn on_construct(_entity: &mut EntityCommands) {}
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiExtensionError {
    #[error("required child path {0:?} was not found")]
    MissingChild(String),
    #[error("{0}")]
    Construction(String),
}

/// Construction result on the control view, independent of list membership.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiExtensionState {
    resource_url: String,
    extension: &'static str,
    error: Option<GuiExtensionError>,
}
impl GuiExtensionState {
    pub fn resource_url(&self) -> &str {
        &self.resource_url
    }
    pub fn extension_type(&self) -> &'static str {
        self.extension
    }
    pub fn error(&self) -> Option<&GuiExtensionError> {
        self.error.as_ref()
    }
}

type Constructor = fn(&mut Commands, Entity, &GuiQuery) -> Option<GuiExtensionError>;
#[derive(Clone, Copy)]
struct Registration {
    name: &'static str,
    construct: Constructor,
}

#[derive(Resource, Default)]
pub(crate) struct Extensions(HashMap<String, Registration>);
#[derive(Component)]
pub(crate) struct Resolved;

pub trait FairyGuiAppExt {
    /// Register by native ui://package_id+item_id or ui://package_name/item_name.
    /// ID registrations take precedence over names, as in UIObjectFactory.
    /// Replacing a registration affects future instances, preserving live ones.
    fn register_gui_extension<T: GuiExtension>(&mut self) -> &mut Self;
    /// Bind the same Rust extension to another exported component resource.
    fn register_gui_extension_at<T: GuiExtension>(&mut self, url: impl Into<String>) -> &mut Self;
}
impl FairyGuiAppExt for App {
    fn register_gui_extension<T: GuiExtension>(&mut self) -> &mut Self {
        self.register_gui_extension_at::<T>(T::URL)
    }
    fn register_gui_extension_at<T: GuiExtension>(&mut self, url: impl Into<String>) -> &mut Self {
        let url = url.into();
        assert!(
            url.starts_with("ui://") && url.len() > 5,
            "GuiExtension URL must be a nonempty ui:// reference"
        );
        self.init_resource::<Extensions>();
        self.world_mut().resource_mut::<Extensions>().0.insert(
            url,
            Registration {
                name: std::any::type_name::<T>(),
                construct: |commands, entity, gui| match T::construct(entity, gui) {
                    Ok(component) => {
                        let mut target = commands.entity(entity);
                        target.insert(component);
                        T::on_construct(&mut target);
                        None
                    }
                    Err(error) => Some(error),
                },
            },
        );
        self
    }
}

/// Runs after snapshots at each materialization boundary. A partially spawned
/// subtree waits, while permanent field-binding errors are reported just once.
pub(crate) fn construct(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    registry: Option<Res<Extensions>>,
    views: Query<(Entity, &GObjectRef), Without<Resolved>>,
    gui: GuiQuery,
) {
    let mut pending = Vec::new();
    for (entity, reference) in &views {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let registration = (|| {
            let object = runtime.store().object(reference.0)?;
            object.control().component()?;
            let package = runtime.registry().get_by_id(object.source_package_id()?)?;
            let item = package.item_by_id(object.package_item_id()?)?;
            let url = format!("ui://{}{}", package.id(), item.id);
            let alias = format!("ui://{}/{}", package.name(), item.name);
            let registry = registry.as_ref()?;
            let registration = registry.0.get(&url).or_else(|| registry.0.get(&alias))?;
            Some((url, *registration))
        })();
        if let Some((url, registration)) = registration {
            pending.push((entity, reference.0, url, registration));
        } else {
            commands.entity(entity).insert(Resolved);
        }
    }
    // Native onConstruct completes children before invoking their parent's hook.
    pending.sort_by_key(|(_, object, _, _)| {
        let mut depth = 0;
        let mut parent = runtime
            .store()
            .object(*object)
            .and_then(|object| object.parent());
        while let Some(id) = parent {
            depth += 1;
            parent = runtime
                .store()
                .object(id)
                .and_then(|object| object.parent());
        }
        std::cmp::Reverse(depth)
    });
    for (entity, id, url, registration) in pending {
        let Some(object) = runtime.store().object(id) else {
            continue;
        };
        let mut subtree = object.children().to_vec();
        let mut ready = gui.get(entity).is_some();
        while let Some(id) = subtree.pop() {
            let Some(child) = runtime.store().object(id) else {
                continue;
            };
            ready &= runtime
                .entity_of(id)
                .is_some_and(|view| gui.get(view).is_some());
            subtree.extend_from_slice(child.children());
        }
        if !ready {
            continue;
        }
        commands.entity(entity).insert(Resolved);
        let error = (registration.construct)(&mut commands, entity, &gui);
        commands.entity(entity).insert(GuiExtensionState {
            resource_url: url,
            extension: registration.name,
            error,
        });
    }
}
