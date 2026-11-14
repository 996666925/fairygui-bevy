//! Writable ECS properties with native-value baselines to prevent feedback.
use crate::{prelude::*, runtime::GuiRuntime};
use bevy::ecs::query::QueryState;
use bevy::prelude::*;
use fairygui_core::{object::GObjectId, widget::ControlData};

pub(crate) trait Property:
    Component<Mutability = bevy::ecs::component::Mutable> + Clone + PartialEq
{
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self>;
    /// Keep application writes pending while dependent content is prepared.
    fn ready(_world: &World, _entity: Entity, _object: GObjectId) -> bool {
        true
    }
    /// Add read-only metadata owned by the materialized ECS view.
    fn read_view(&mut self, _world: &World, _object: GObjectId) {}
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId);
}

#[derive(Component)]
pub(crate) struct NativeValue<T: Property> {
    object: GObjectId,
    value: T,
}

type PropertyEntities<T> = QueryState<
    Entity,
    (
        Or<(With<GObjectRef>, With<T>)>,
        Without<crate::graph::GraphSurface>,
    ),
>;
type PropertyValues<T> = QueryState<(
    Option<&'static GObjectRef>,
    Option<&'static T>,
    Option<&'static NativeValue<T>>,
)>;

#[derive(Resource)]
struct PropertyQuery<T: Property> {
    entities: PropertyEntities<T>,
    values: PropertyValues<T>,
}

impl<T: Property> FromWorld for PropertyQuery<T> {
    fn from_world(world: &mut World) -> Self {
        Self {
            entities: world.query_filtered(),
            values: world.query(),
        }
    }
}

/// Apply only fields changed by application code, then publish the current
/// model value. Input, animation and relation changes never replay old writes.
pub(crate) fn sync(world: &mut World) {
    // Controller gears can change every property below. Consume application
    // page edits before publishing geometry, values, text and display state.
    sync_property::<GuiControllers>(world);
    sync_property::<GuiData>(world);
    sync_property::<GuiButtonSettings>(world);
    sync_property::<GuiScrollSettings>(world);
    sync_property::<GuiListLayout>(world);
    sync_property::<GuiListSettings>(world);
    sync_property::<GuiListSelection>(world);
    sync_property::<GuiTreeNode>(world);
    sync_property::<GuiTreeSettings>(world);
    sync_property::<GuiBlendMode>(world);
    sync_property::<GuiColorFilter>(world);
    sync_property::<GuiOpaque>(world);
    sync_property::<GuiColor>(world);
    sync_property::<GuiGraph>(world);
    sync_property::<GuiTextFormat>(world);
    sync_property::<GuiTooltips>(world);
    sync_property::<GuiTextInputSettings>(world);
    sync_property::<GuiLoaderSettings>(world);
    sync_property::<GuiImageFill>(world);
    sync_property::<GuiRenderOrder>(world);
    sync_property::<GuiImageFlip>(world);
    sync_property::<GuiSortingOrder>(world);
    sync_property::<GuiTransform>(world);
    sync_property::<GuiDisplay>(world);
    sync_property::<GuiText>(world);
    sync_property::<GuiIcon>(world);
    sync_property::<GuiAnimationState>(world);
    sync_property::<GuiButtonSelected>(world);
    sync_property::<GuiSliderState>(world);
    sync_property::<GuiSliderValue>(world);
    sync_property::<GuiProgressState>(world);
    sync_property::<GuiProgressValue>(world);
    sync_property::<GuiComboBoxState>(world);
    // Row button edits also change a list's native selected indices.
    sync_property::<GuiListSelection>(world);
    if world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .flush_list_layouts()
        != 0
    {
        // All row edits have now been applied. Publish positions and automatic
        // widths again so earlier entity visits cannot expose stale geometry.
        sync_property::<GuiTransform>(world);
        sync_property::<GuiDisplay>(world);
    }
}

fn snapshot<T: Property>(world: &World, object: GObjectId) -> Option<T> {
    let mut value = T::read(world.non_send::<GuiRuntime>(), object)?;
    value.read_view(world, object);
    Some(value)
}

pub(crate) fn sync_property<T: Property>(world: &mut World) {
    world.init_resource::<PropertyQuery<T>>();
    world.resource_scope(|world, mut query: Mut<PropertyQuery<T>>| {
        // Keep archetype matches across passes, but release the iterator before
        // projecting components. Insert observers may move or despawn entities.
        let entities: Vec<_> = query.entities.iter(world).collect();
        for entity in entities {
            let Ok((reference, value, stored)) = query.values.get(world, entity) else {
                continue;
            };
            // Read each candidate now: an earlier projection's observer may have
            // changed its components since we collected the candidate entities.
            let reference = reference.map(|r| r.0);
            let value = value.cloned();
            let stored = stored.map(|p| (p.object, p.value.clone()));
            let runtime = world.get_non_send::<GuiRuntime>().unwrap();
            let object = reference
                .or_else(|| runtime.ecs_root_object(entity))
                .or_else(|| runtime.ecs_window_object(entity));
            let Some(object) = object else { continue };
            let Some(native) = snapshot::<T>(world, object) else {
                if value.is_some() || stored.is_some() {
                    world.entity_mut(entity).remove::<(T, NativeValue<T>)>();
                }
                continue;
            };
            if !T::ready(world, entity, object) {
                continue;
            }
            let baseline = stored
                .as_ref()
                // View components are projections of their current binding.
                // Root/window owners instead retain application configuration
                // when replacing their underlying native model.
                .filter(|(id, _)| reference.is_some() || *id == object)
                .map_or(&native, |(_, value)| value);
            let next = if let Some(value) = &value
                && *value != native
                && value != baseline
            {
                value.apply(
                    baseline,
                    &native,
                    &mut world.get_non_send_mut::<GuiRuntime>().unwrap(),
                    object,
                );
                // Setters may clamp values or update read-only player metadata.
                snapshot::<T>(world, object).unwrap_or(native)
            } else {
                native
            };
            if value.as_ref() != Some(&next) {
                world.entity_mut(entity).insert(next.clone());
            }
            if stored
                .as_ref()
                .is_none_or(|(id, value)| *id != object || *value != next)
            {
                world.entity_mut(entity).insert(NativeValue {
                    object,
                    value: next,
                });
            }
        }
    });
}

macro_rules! field {
    ($value:ident, $previous:ident, $native:ident, $field:ident, $body:block) => {
        if $value.$field != $previous.$field && $value.$field != $native.$field $body
    };
}

impl Property for GuiText {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().object_text(object).map(Self::new)
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_object_text(object, &self.0);
    }
}

impl Property for GuiSortingOrder {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime
            .store()
            .object(object)
            .map(|o| Self(o.sorting_order()))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_sorting_order(object, self.0);
    }
}

impl Property for GuiRenderOrder {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self {
            order: runtime.store().children_render_order(object)?,
            apex_index: runtime.store().apex_index(object)?,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        field!(self, previous, native, order, {
            runtime
                .store_mut()
                .set_children_render_order(object, self.order);
        });
        field!(self, previous, native, apex_index, {
            runtime.store_mut().set_apex_index(object, self.apex_index);
        });
    }
}
impl Property for GuiIcon {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let icon = match runtime.store().object(object)?.control() {
            ControlData::Loader(data) => data.url.as_deref(),
            ControlData::Button(data) => data.icon.as_deref(),
            ControlData::Label(data) => data.icon.as_deref(),
            ControlData::ComboBox(data) => data.icon.as_deref(),
            _ => return None,
        };
        Some(Self::new(icon.unwrap_or_default()))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().apply_icon_to(object, &self.0);
    }
}
impl Property for GuiTransform {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().object(object).map(Self::from_object)
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        field!(self, previous, native, size, {
            if self.size.is_finite() && self.size.min_element() >= 0. {
                runtime
                    .store_mut()
                    .set_size(object, self.size.x, self.size.y);
            }
        });
        field!(self, previous, native, position, {
            if self.position.is_finite() {
                runtime
                    .store_mut()
                    .set_xy(object, self.position.x, self.position.y);
            }
        });
        field!(self, previous, native, scale, {
            if self.scale.is_finite() {
                runtime
                    .store_mut()
                    .set_scale(object, self.scale.x, self.scale.y);
            }
        });
        field!(self, previous, native, skew, {
            if self.skew.is_finite() {
                runtime
                    .store_mut()
                    .set_skew(object, self.skew.x, self.skew.y);
            }
        });
        if (self.pivot, self.pivot_as_anchor) != (previous.pivot, previous.pivot_as_anchor)
            && (self.pivot, self.pivot_as_anchor) != (native.pivot, native.pivot_as_anchor)
            && self.pivot.is_finite()
        {
            runtime
                .store_mut()
                .set_pivot(object, self.pivot.x, self.pivot.y, self.pivot_as_anchor);
        }
        field!(self, previous, native, rotation, {
            if self.rotation.is_finite() {
                runtime.store_mut().set_rotation(object, self.rotation);
            }
        });
    }
}
impl Property for GuiDisplay {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let o = runtime.store().object(object)?;
        Some(Self {
            visible: o.visible(),
            display_visible: runtime.store().display_visible(object),
            on_stage: runtime.store().is_on_stage(object),
            alpha: o.alpha(),
            effective_alpha: runtime.store().effective_alpha(object),
            grayed: o.grayed(),
            touchable: o.touchable(),
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        field!(self, previous, native, visible, {
            runtime.store_mut().set_visible(object, self.visible);
        });
        field!(self, previous, native, alpha, {
            if self.alpha.is_finite() {
                runtime.store_mut().set_alpha(object, self.alpha);
            }
        });
        field!(self, previous, native, grayed, {
            runtime.store_mut().set_grayed(object, self.grayed);
        });
        field!(self, previous, native, touchable, {
            runtime.store_mut().set_touchable(object, self.touchable);
        });
    }
}
impl Property for GuiButtonSelected {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().button_selected(object).map(Self)
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_button_selected(object, self.0);
    }
}
impl Property for GuiSliderValue {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().slider_value(object).map(Self)
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_slider_value(object, self.0);
    }
}
impl Property for GuiProgressValue {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().progress_value(object).map(Self)
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_progress(object, self.0);
    }
}
impl Property for GuiSliderState {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::Slider(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        Some(Self {
            min: data.min,
            max: data.max,
            value: data.value,
            whole_numbers: data.whole_numbers,
            reverse: data.reverse,
            change_on_click: data.change_on_click,
            can_drag: data.can_drag,
            title_type: data.title_type,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let min = if self.min != previous.min {
            self.min
        } else {
            native.min
        };
        let max = if self.max != previous.max {
            self.max
        } else {
            native.max
        };
        if (min, max) != (native.min, native.max) {
            runtime.store_mut().set_slider_range(object, min, max);
        }
        field!(self, previous, native, whole_numbers, {
            runtime
                .store_mut()
                .set_slider_whole_numbers(object, self.whole_numbers);
        });
        field!(self, previous, native, change_on_click, {
            runtime
                .store_mut()
                .set_slider_change_on_click(object, self.change_on_click);
        });
        field!(self, previous, native, can_drag, {
            runtime
                .store_mut()
                .set_slider_can_drag(object, self.can_drag);
        });
        field!(self, previous, native, title_type, {
            runtime
                .store_mut()
                .set_slider_title_type(object, self.title_type);
        });
        field!(self, previous, native, value, {
            runtime.store_mut().set_slider_value(object, self.value);
        });
    }
}
impl Property for GuiProgressState {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::ProgressBar(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        Some(Self {
            min: data.min,
            max: data.max,
            value: data.value,
            displayed_value: runtime.store().progress_display_value(object)?,
            title_type: data.title_type,
            reverse: data.reverse,
            tweening: runtime.store().progress_is_tweening(object),
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let min = if self.min != previous.min {
            self.min
        } else {
            native.min
        };
        let max = if self.max != previous.max {
            self.max
        } else {
            native.max
        };
        if (min, max) != (native.min, native.max) {
            runtime.store_mut().set_progress_range(object, min, max);
        }
        field!(self, previous, native, title_type, {
            runtime
                .store_mut()
                .set_progress_title_type(object, self.title_type);
        });
        field!(self, previous, native, value, {
            runtime.store_mut().set_progress(object, self.value);
        });
    }
}

impl Property for GuiComboBoxState {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::ComboBox(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        let popup = runtime.store().combo_popup(object);
        Some(Self {
            items: data.items.clone(),
            values: data.values.clone(),
            icons: data.icons.clone(),
            selected_index: data.selected_index,
            visible_item_count: data.visible_item_count,
            direction: match data.popup_direction {
                1 => GuiPopupDirection::Up,
                2 => GuiPopupDirection::Down,
                _ => GuiPopupDirection::Auto,
            },
            selection_controller: runtime
                .store()
                .combo_selection_controller(object)
                .map(str::to_owned),
            dropdown: popup.as_ref().and_then(|p| runtime.entity_of(p.dropdown)),
            list: popup.as_ref().and_then(|p| runtime.entity_of(p.list)),
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        field!(self, previous, native, items, {
            runtime
                .store_mut()
                .set_combo_items(object, self.items.clone());
        });
        field!(self, previous, native, values, {
            runtime
                .store_mut()
                .set_combo_values(object, self.values.clone());
        });
        field!(self, previous, native, icons, {
            runtime
                .store_mut()
                .set_combo_icons(object, self.icons.clone());
        });
        // Bind the requested controller before selecting an item, so a combined
        // component edit updates the new reference. Items may have clamped the
        // native index above even if it matched this request before application.
        field!(self, previous, native, selection_controller, {
            runtime
                .store_mut()
                .set_combo_selection_controller(object, self.selection_controller.as_deref());
        });
        if self.selected_index != previous.selected_index {
            runtime
                .store_mut()
                .select_combo_index(object, self.selected_index);
        }
        field!(self, previous, native, visible_item_count, {
            runtime
                .store_mut()
                .set_combo_visible_item_count(object, self.visible_item_count);
        });
        field!(self, previous, native, direction, {
            runtime.store_mut().set_combo_popup_direction(
                object,
                match self.direction {
                    GuiPopupDirection::Auto => 0,
                    GuiPopupDirection::Up => 1,
                    GuiPopupDirection::Down => 2,
                },
            );
        });
    }
}

#[cfg(test)]
#[path = "ecs_properties_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ecs_child_order_tests.rs"]
mod child_order_tests;

#[cfg(test)]
#[path = "ecs_controller_tests.rs"]
mod controller_tests;

#[cfg(test)]
#[path = "ecs_render_order_tests.rs"]
mod render_order_tests;
