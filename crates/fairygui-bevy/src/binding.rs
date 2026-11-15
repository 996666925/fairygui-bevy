//! Checked entity handles used by `fairygui-build` generated bindings.
use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use bevy::prelude::*;
use fairygui_core::package::ObjectType;

use crate::{GuiControllerState, GuiControllers, GuiMut, GuiQuery, GuiTransitionState};

/// Kind and optional exact source template expected by a generated handle.
pub trait GuiType: Send + Sync + 'static {
    const KIND: ObjectType;
    const RESOURCE: Option<(&'static str, &'static str)> = None;
}

/// A generated component binding. Bind after materialization (for example in
/// Initialize or after Sync); an owner or its control view is accepted.
pub trait GuiBinding: GuiType + Component + Sized {
    const URL: &'static str;
    const ITEM_NAME: &'static str;
    fn bind(gui: &GuiQuery, entity: Entity) -> Result<Self, GuiBindingError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiBindingError {
    #[error("GUI entity {0} is not ready or no longer exists")]
    NotReady(Entity),
    #[error("GUI entity {entity} has kind {actual:?}, expected {expected:?}")]
    WrongKind {
        entity: Entity,
        expected: ObjectType,
        actual: ObjectType,
    },
    #[error("GUI entity {entity} does not use template ui://{package}{item}")]
    WrongResource {
        entity: Entity,
        package: &'static str,
        item: &'static str,
    },
    #[error("GUI entity {parent} has no direct child named {name:?}")]
    MissingChild { parent: Entity, name: &'static str },
    #[error("GUI entity {entity} has no controller named {name:?}")]
    MissingController { entity: Entity, name: &'static str },
    #[error("GUI controller {name:?} on {entity} has a different exported page table")]
    ControllerSchema { entity: Entity, name: &'static str },
    #[error("GUI entity {entity} has no transition named {name:?}")]
    MissingTransition { entity: Entity, name: &'static str },
}

/// A validated control view entity. This does not own its lifetime: removing or
/// replacing the UI invalidates handles, just like ordinary Bevy Entity values.
/// There is deliberately no unchecked public constructor or implicit cast.
///
/// ```compile_fail
/// use fairygui_bevy::{GuiHandle, binding::kind};
/// fn wrong(button: GuiHandle<kind::Button>) -> GuiHandle<kind::Text> {
///     button
/// }
/// ```
#[derive(Debug)]
pub struct GuiHandle<T: GuiType> {
    entity: Entity,
    marker: PhantomData<fn() -> T>,
}
impl<T: GuiType> Copy for GuiHandle<T> {}
impl<T: GuiType> PartialEq for GuiHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}
impl<T: GuiType> Eq for GuiHandle<T> {}
impl<T: GuiType> Hash for GuiHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entity.hash(state);
    }
}
impl<T: GuiType> Clone for GuiHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: GuiType> GuiHandle<T> {
    pub fn new(gui: &GuiQuery, entity: Entity) -> Result<Self, GuiBindingError> {
        let info = gui.get(entity).ok_or(GuiBindingError::NotReady(entity))?;
        let entity = gui.view(entity).ok_or(GuiBindingError::NotReady(entity))?;
        if info.kind() != T::KIND {
            return Err(GuiBindingError::WrongKind {
                entity,
                expected: T::KIND,
                actual: info.kind(),
            });
        }
        if let Some((package, item)) = T::RESOURCE
            && (info.package_id() != Some(package) || info.package_item_id() != Some(item))
        {
            return Err(GuiBindingError::WrongResource {
                entity,
                package,
                item,
            });
        }
        Ok(Self {
            entity,
            marker: PhantomData,
        })
    }
    pub fn child(
        gui: &GuiQuery,
        parent: Entity,
        name: &'static str,
    ) -> Result<Self, GuiBindingError> {
        let child = gui
            .child_by_name(parent, name)
            .ok_or(GuiBindingError::MissingChild { parent, name })?;
        Self::new(gui, child)
    }
    pub fn entity(self) -> Entity {
        self.entity
    }
    pub fn validate(self, gui: &GuiQuery) -> Result<(), GuiBindingError> {
        Self::new(gui, self.entity).map(|_| ())
    }
}
impl<T: GuiBinding> GuiHandle<T> {
    /// Bind a referenced exported component's own typed fields on demand.
    pub fn bind(self, gui: &GuiQuery) -> Result<T, GuiBindingError> {
        T::bind(gui, self.entity)
    }
}

/// Page IDs and names in exported order. Generated enums are controller-specific,
/// so a page from a different controller cannot be selected accidentally.
pub trait GuiPage: Copy + Send + Sync + 'static {
    const PAGES: &'static [(&'static str, &'static str)];
    fn id(self) -> &'static str;
    fn from_id(id: &str) -> Option<Self>;
}
/// A controller accepts only its generated page enum.
///
/// ```compile_fail
/// use bevy::prelude::*;
/// use fairygui_bevy::{GuiController, GuiControllers, GuiPage};
/// fn wrong<A: GuiPage, B: GuiPage>(controller: GuiController<A>,
///     page: B, controllers: &mut Query<&mut GuiControllers>) {
///     controller.select(controllers, page);
/// }
/// ```
#[derive(Debug)]
pub struct GuiController<P: GuiPage> {
    entity: Entity,
    name: &'static str,
    marker: PhantomData<fn() -> P>,
}
impl<P: GuiPage> Copy for GuiController<P> {}
impl<P: GuiPage> Clone for GuiController<P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<P: GuiPage> GuiController<P> {
    pub fn new(
        gui: &GuiQuery,
        entity: Entity,
        name: &'static str,
    ) -> Result<Self, GuiBindingError> {
        let entity = gui.view(entity).ok_or(GuiBindingError::NotReady(entity))?;
        let state = gui
            .controller(entity, name)
            .ok_or(GuiBindingError::MissingController { entity, name })?;
        let handle = Self {
            entity,
            name,
            marker: PhantomData,
        };
        handle.check(state)?;
        Ok(handle)
    }
    fn check(&self, state: &GuiControllerState) -> Result<(), GuiBindingError> {
        if state.page_ids().len() == P::PAGES.len()
            && state.page_names().len() == P::PAGES.len()
            && state
                .page_ids()
                .iter()
                .zip(state.page_names())
                .map(|(id, name)| (id.as_str(), name.as_str()))
                .eq(P::PAGES.iter().copied())
        {
            Ok(())
        } else {
            Err(GuiBindingError::ControllerSchema {
                entity: self.entity,
                name: self.name,
            })
        }
    }
    pub fn entity(self) -> Entity {
        self.entity
    }
    pub fn name(self) -> &'static str {
        self.name
    }
    pub fn state<'a>(&self, gui: &'a GuiQuery) -> Option<&'a GuiControllerState> {
        gui.controller(self.entity, self.name)
    }
    pub fn selected(&self, gui: &GuiQuery) -> Option<P> {
        P::from_id(self.state(gui)?.selected_page_id()?)
    }
    /// Edit the native writable ECS controller on the bound entity.
    pub fn select(
        &self,
        controllers: &mut Query<&mut GuiControllers>,
        page: P,
    ) -> Result<bool, GuiBindingError> {
        let mut controllers = controllers
            .get_mut(self.entity)
            .map_err(|_| GuiBindingError::NotReady(self.entity))?;
        let state = controllers
            .get_mut(self.name)
            .ok_or(GuiBindingError::MissingController {
                entity: self.entity,
                name: self.name,
            })?;
        self.check(state)?;
        Ok(state.set_selected_page_id(page.id()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GuiTransition {
    entity: Entity,
    name: &'static str,
}
impl GuiTransition {
    pub fn new(
        gui: &GuiQuery,
        entity: Entity,
        name: &'static str,
    ) -> Result<Self, GuiBindingError> {
        let entity = gui.view(entity).ok_or(GuiBindingError::NotReady(entity))?;
        gui.transition(entity, name)
            .ok_or(GuiBindingError::MissingTransition { entity, name })?;
        Ok(Self { entity, name })
    }
    pub fn entity(self) -> Entity {
        self.entity
    }
    pub fn name(self) -> &'static str {
        self.name
    }
    pub fn state<'a>(&self, gui: &'a GuiQuery) -> Option<&'a GuiTransitionState> {
        gui.transition(self.entity, self.name)
    }
    pub fn play(self, gui: &mut GuiMut, times: i32) -> bool {
        let Some(mut object) = gui.get_mut(self.entity) else {
            return false;
        };
        object.play_transition(self.name, times);
        true
    }
}

/// Marker types for inline controls. Exported component references instead use
/// their generated binding type as a marker, preserving exact template identity.
pub mod kind {
    use super::{GuiType, ObjectType};
    macro_rules! kinds { ($($name:ident),* $(,)?) => { $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;
        impl GuiType for $name { const KIND: ObjectType = ObjectType::$name; }
    )* }; }
    kinds!(
        Image,
        MovieClip,
        Swf,
        Graph,
        Loader,
        Group,
        Text,
        RichText,
        InputText,
        Component,
        List,
        Label,
        Button,
        ComboBox,
        ProgressBar,
        Slider,
        ScrollBar,
        Tree,
        Loader3D
    );
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Extension<const CODE: u8>;
    impl<const CODE: u8> GuiType for Extension<CODE> {
        const KIND: ObjectType = ObjectType::Extension(CODE);
    }
}
