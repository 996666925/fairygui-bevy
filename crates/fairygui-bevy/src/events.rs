//! Bevy observer propagation over a captured FairyGUI logical path.
use crate::runtime::{GObjectRef, GuiRuntime};
use bevy::{
    ecs::{
        event::{PropagateEntityTrigger, SetEntityEventTarget},
        query::QueryData,
        traversal::Traversal,
    },
    prelude::*,
};
/// Payload shared by native and application-defined GUI notifications.
pub use fairygui_core::object::EventData as GuiEventData;
use fairygui_core::object::{EventKind, GObjectId, UiEvent};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[cfg(test)]
#[path = "ecs_event_tests.rs"]
mod tests;

pub(crate) struct PendingGuiEvent {
    pub event: UiEvent,
    pub route: Vec<GObjectId>,
}
impl PendingGuiEvent {
    pub fn direct(event: UiEvent) -> Self {
        Self {
            route: vec![event.current_target],
            event,
        }
    }
}

/// Shared payload of typed FairyGUI notifications. Observe GuiClick,
/// GuiMouseDown, GuiStateChanged, etc. instead of filtering a kind in a callback.
/// Each typed notification dereferences to this data and follows one captured
/// logical path. `event.propagate(false)` stops that dispatch at this recipient.
#[derive(Debug, Clone)]
pub struct GuiEvent {
    /// Current recipient, updated together with event.current_target.
    pub entity: Entity,
    /// Original control view, independent of the current recipient. None if
    /// the original native object did not have a view when Publish resolved it.
    pub target: Option<Entity>,
    pub event: UiEvent,
    route: Arc<[(Entity, GObjectId)]>,
}
impl GuiEvent {
    /// Construct a direct notification for one entity. To capture the native
    /// logical bubbling path, enqueue through GuiRuntime::dispatch_event.
    fn new(entity: Entity, event: UiEvent) -> Self {
        Self {
            entity,
            target: event.is_at_target().then_some(entity),
            route: Arc::from([(entity, event.current_target)]),
            event,
        }
    }
}
impl GuiEvent {
    fn set_recipient(&mut self, entity: Entity) {
        self.entity = entity;
        if let Some((_, object)) = self
            .route
            .iter()
            .find(|(recipient, _)| *recipient == entity)
        {
            self.event.current_target = *object;
            self.event.bubbling = *object != self.event.target;
        }
    }
}

impl bevy::ecs::entity::ContainsEntity for GuiEvent {
    fn entity(&self) -> Entity {
        self.entity
    }
}

/// Common metadata interface for typed observers and generic event recorders.
/// Built-in kinds are dispatched once, through their corresponding event type.
pub trait GuiEventType: EntityEvent + Deref<Target = GuiEvent> + DerefMut {
    fn context(&self) -> &GuiEvent {
        self
    }
}

macro_rules! event_types {
    ($($name:ident => $kind:ident),* $(,)?) => {
        $(
            #[doc = concat!("Typed `", stringify!($kind), "` notification with logical bubbling.")]
            #[derive(EntityEvent, Debug, Clone)]
            #[entity_event(trigger = PropagateEntityTrigger<true, Self, GuiEventTraversal>)]
            pub struct $name { #[event_target] context: GuiEvent }
            impl $name {
                /// Construct a notification delivered only to this entity.
                /// Runtime dispatch captures the full native bubbling path.
                pub fn new(entity: Entity, object: GObjectId) -> Self {
                    Self { context: GuiEvent::new(entity, UiEvent::new(EventKind::$kind, object)) }
                }
            }
            impl Deref for $name { type Target = GuiEvent; fn deref(&self) -> &GuiEvent { &self.context } }
            impl DerefMut for $name { fn deref_mut(&mut self) -> &mut GuiEvent { &mut self.context } }
            impl GuiEventType for $name {}
            impl SetEntityEventTarget for $name {
                fn set_event_target(&mut self, entity: Entity) { self.context.set_recipient(entity); }
            }
        )*
        pub(crate) fn trigger_typed(commands: &mut Commands, context: GuiEvent) {
            match context.event.kind {
                $(EventKind::$kind => commands.trigger($name { context }),)*
                EventKind::Other(_) => commands.trigger(GuiCustomEvent { context }),
            }
        }
        #[cfg(test)]
        #[test]
        fn each_native_kind_notifies_only_its_matching_observer_once() {
            #[derive(Resource, Default)]
            struct Seen(Vec<EventKind>);
            let mut app = App::new();
            app.init_non_send::<GuiRuntime>().init_resource::<Seen>()
                .add_systems(Update, emit_gui_events);
            let id = app.world_mut().non_send_mut::<GuiRuntime>().store_mut()
                .create(fairygui_core::package::ObjectType::Component);
            let entity = app.world_mut().spawn(GObjectRef(id)).id();
            $(app.world_mut().entity_mut(entity).observe(|event: On<$name>, mut seen: ResMut<Seen>| {
                assert_eq!(event.event.kind, EventKind::$kind);
                seen.0.push(EventKind::$kind);
            });)*
            let expected = vec![$(EventKind::$kind,)*];
            for kind in expected.iter().cloned() {
                app.world_mut().non_send_mut::<GuiRuntime>().dispatch(kind, id);
            }
            app.update();
            assert_eq!(app.world().resource::<Seen>().0, expected);
        }
    };
}
event_types! {
    GuiClick => Click, GuiRightClick => RightClick,
    GuiRightMouseDown => RightMouseDown, GuiRightMouseUp => RightMouseUp,
    GuiMouseMove => MouseMove, GuiMouseDrag => MouseDrag, GuiMouseDragEnd => MouseDragEnd,
    GuiLinkEvent => Link, GuiMouseDown => MouseDown, GuiMouseUp => MouseUp,
    GuiRollOver => RollOver, GuiRollOut => RollOut, GuiMouseWheel => MouseWheel,
    GuiTouchBegin => TouchBegin, GuiTouchMove => TouchMove, GuiTouchEnd => TouchEnd,
    GuiKeyDown => KeyDown, GuiKeyUp => KeyUp,
    GuiFocusIn => FocusIn, GuiFocusOut => FocusOut,
    GuiInputEvent => Input, GuiChange => Change, GuiEnter => Enter,
    GuiWindowShown => WindowShown, GuiWindowHide => WindowHide,
    GuiXyChanged => XyChanged, GuiSizeChanged => SizeChanged, GuiSizeDelayChange => SizeDelayChange,
    GuiStateChanged => StateChanged, GuiGearStop => GearStop,
    GuiClickItem => ClickItem, GuiScroll => Scroll, GuiScrollEnd => ScrollEnd,
    GuiDragStart => DragStart, GuiDragMove => DragMove, GuiDragEnd => DragEnd, GuiDropEvent => Drop,
    GuiPullDownRelease => PullDownRelease, GuiPullUpRelease => PullUpRelease,
}

/// A dynamically named FairyGUI `Other(name)` notification.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = PropagateEntityTrigger<true, Self, GuiEventTraversal>)]
pub struct GuiCustomEvent {
    #[event_target]
    context: GuiEvent,
}
impl GuiCustomEvent {
    pub fn new(entity: Entity, object: GObjectId, name: impl Into<String>) -> Self {
        Self {
            context: GuiEvent::new(entity, UiEvent::new(EventKind::Other(name.into()), object)),
        }
    }
    pub fn name(&self) -> &str {
        match &self.event.kind {
            EventKind::Other(name) => name,
            _ => unreachable!(),
        }
    }
}
impl Deref for GuiCustomEvent {
    type Target = GuiEvent;
    fn deref(&self) -> &GuiEvent {
        &self.context
    }
}
impl DerefMut for GuiCustomEvent {
    fn deref_mut(&mut self) -> &mut GuiEvent {
        &mut self.context
    }
}
impl GuiEventType for GuiCustomEvent {}
impl SetEntityEventTarget for GuiCustomEvent {
    fn set_event_target(&mut self, entity: Entity) {
        self.context.set_recipient(entity);
    }
}

macro_rules! text_value {
    ($($name:ident),*) => { $(impl $name {
        /// Current committed input text, without password masking or IME preedit.
        pub fn value(&self) -> &str {
            match &self.event.data { fairygui_core::object::EventData::Text(value) => value, _ => "" }
        }
    })* };
}
text_value!(GuiInputEvent, GuiChange, GuiEnter);

/// Query used by Bevy's propagation trigger. Logical paths live in the event,
/// so decorative render hierarchies and intervening reparenting cannot reroute it.
#[doc(hidden)]
#[derive(QueryData, Debug)]
pub struct GuiEventTraversal {
    entity: Entity,
}
impl<E: GuiEventType> Traversal<E> for GuiEventTraversal {
    fn traverse(item: Self::Item<'_, '_>, event: &E) -> Option<Entity> {
        let index = event
            .route
            .iter()
            .position(|(entity, _)| *entity == item.entity)?;
        event.route.get(index + 1).map(|(entity, _)| *entity)
    }
}

/// Publish one Bevy event per native dispatch, with observer-controlled bubbling.
pub fn emit_gui_events(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    entities: Query<(Entity, &GObjectRef)>,
) {
    for pending in runtime.take_gui_events() {
        let resolve = |object| {
            runtime
                .entity_of(object)
                .filter(|&entity| {
                    entities
                        .get(entity)
                        .is_ok_and(|(_, reference)| reference.0 == object)
                })
                .or_else(|| {
                    entities
                        .iter()
                        .filter_map(|(entity, reference)| (reference.0 == object).then_some(entity))
                        .min_by_key(|entity| entity.to_bits())
                })
        };
        let target = resolve(pending.event.target);
        let route: Arc<[(Entity, GObjectId)]> = pending
            .route
            .into_iter()
            .filter_map(|object| resolve(object).map(|entity| (entity, object)))
            .collect();
        let Some(&(entity, current_target)) = route.first() else {
            continue;
        };
        let mut event = pending.event;
        event.current_target = current_target;
        event.bubbling = current_target != event.target;
        trigger_typed(
            &mut commands,
            GuiEvent {
                entity,
                target,
                event,
                route,
            },
        );
    }
}
