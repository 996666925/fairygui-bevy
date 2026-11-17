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
#[path = "ecs_drag_event_tests.rs"]
mod drag_tests;
#[cfg(test)]
#[path = "ecs_release_event_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "ecs_event_tests.rs"]
mod tests;

pub(crate) struct PendingGuiEvent {
    pub event: UiEvent,
    pub route: Vec<GObjectId>,
    pub release_tail: Vec<GObjectId>,
}
impl PendingGuiEvent {
    pub fn direct(event: UiEvent) -> Self {
        Self {
            route: vec![event.current_target],
            event,
            release_tail: Vec::new(),
        }
    }
}

/// Native MouseUp always enters the first saved press recipient omitted from
/// the hit path, even when the hit-path observer stopped propagation. The stop
/// flag is carried into that second phase. Safe Bevy triggers let us retain
/// that flag without implementing a custom unsafe Trigger.
fn trigger_release<E>(
    commands: &mut Commands,
    context: GuiEvent,
    tail: Arc<[(Entity, GObjectId)]>,
    first_is_resolved: bool,
    make: fn(GuiEvent) -> E,
) where
    E: GuiEventType + SetEntityEventTarget,
    for<'a> E: Event<Trigger<'a> = PropagateEntityTrigger<true, E, GuiEventTraversal>>,
{
    commands.queue(move |world: &mut World| {
        let mut event = make(context);
        let mut trigger = PropagateEntityTrigger::<true, E, GuiEventTraversal>::default();
        if !event.route.is_empty() {
            world.trigger_ref_with(&mut event, &mut trigger);
        }
        let mut context = event.context().clone();
        if let Some(&(entity, _)) = tail.first()
            && (trigger.propagate || first_is_resolved)
        {
            context.route = tail;
            context.set_recipient(entity);
            let mut event = make(context);
            let mut tail_trigger = PropagateEntityTrigger::<true, E, GuiEventTraversal>::default();
            tail_trigger.propagate = trigger.propagate;
            world.trigger_ref_with(&mut event, &mut tail_trigger);
        }
    });
}

/// Shared payload of typed FairyGUI notifications. Observe GuiClick,
/// GuiMouseDown, GuiStateChanged, etc. instead of filtering a kind in a callback.
/// Each typed notification dereferences to this data and follows one captured
/// logical path. `event.propagate(false)` stops that dispatch at this recipient.
/// Physical MouseUp/RightMouseUp additionally enter their saved press phase:
/// its first omitted press recipient runs even if the hit path stopped, then
/// inherits that stop flag as in native TouchInfo.bubble.
/// Physical RollOver/RollOut notify each changed node directly, with that node
/// as the target. Stopping one notification does not skip another node's event.
#[derive(Debug, Clone)]
pub struct GuiEvent {
    /// Current recipient, updated together with event.current_target.
    pub entity: Entity,
    /// Original control view, independent of the current recipient. For physical
    /// MouseDrag/MouseUp/RightMouseUp this is the current hit, including at saved press
    /// recipients. None for the external stage or a missing original view.
    pub target: Option<Entity>,
    pub event: UiEvent,
    route: Arc<[(Entity, GObjectId)]>,
}
impl GuiEvent {
    pub fn click_count(&self) -> u8 {
        self.event.click_count
    }

    pub fn is_double_click(&self) -> bool {
        self.event.is_double_click()
    }

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
            #[doc = concat!("Typed `", stringify!($kind), "` notification. Runtime supplies its logical delivery route.")]
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
    GuiClick => Click, GuiDoubleClick => DoubleClick, GuiRightClick => RightClick,
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
        let first_saved = pending.release_tail.first().copied();
        let tail: Arc<[(Entity, GObjectId)]> = pending
            .release_tail
            .into_iter()
            .filter(|&object| runtime.store().contains(object))
            .filter_map(|object| resolve(object).map(|entity| (entity, object)))
            .collect();
        let Some(&(entity, current_target)) = route.first().or_else(|| tail.first()) else {
            continue;
        };
        let mut event = pending.event;
        event.current_target = current_target;
        event.bubbling = current_target != event.target;
        let context = GuiEvent {
            entity,
            target,
            event,
            route,
        };
        if !tail.is_empty() {
            let first_is_resolved = tail.first().map(|(_, id)| *id) == first_saved;
            match context.event.kind {
                EventKind::MouseUp => {
                    trigger_release(&mut commands, context, tail, first_is_resolved, |context| {
                        GuiMouseUp { context }
                    })
                }
                EventKind::RightMouseUp => {
                    trigger_release(&mut commands, context, tail, first_is_resolved, |context| {
                        GuiRightMouseUp { context }
                    })
                }
                _ => unreachable!("only pointer releases have a saved press phase"),
            }
        } else {
            trigger_typed(&mut commands, context);
        }
    }
}
