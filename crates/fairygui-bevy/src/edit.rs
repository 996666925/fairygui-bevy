//! Direct object editing for behavior such as playback and scrolling.
use super::*;
use crate::ListNavigationDirection;
use bevy::ecs::system::SystemParam;
use fairygui_core::package::ObjectType;

/// Resolve direct-service targets against the owner's current template. A view
/// keeps its identity until disposal; an owner alias is invalid as soon as its
/// source component is removed or points at a different template.
pub(crate) fn resolve_object(
    runtime: &GuiRuntime,
    entity: Entity,
    reference: Option<&GObjectRef>,
    root: Option<&FairyGuiRoot>,
    window: Option<&windows::FairyGuiWindow>,
) -> Option<GObjectId> {
    reference
        .map(|reference| reference.0)
        .or_else(|| {
            root.filter(|source| runtime.ecs_root_source_matches(entity, source))
                .and_then(|_| runtime.ecs_root_object(entity))
        })
        .or_else(|| {
            window
                .filter(|source| runtime.ecs_window_source_matches(entity, source))
                .and_then(|_| runtime.ecs_window_object(entity))
        })
        .filter(|&object| runtime.store().contains(object))
}

#[derive(SystemParam, Debug)]
pub(crate) struct GuiObjectResolver<'w, 's> {
    references: Query<'w, 's, &'static GObjectRef>,
    roots: Query<'w, 's, &'static FairyGuiRoot>,
    windows: Query<'w, 's, &'static windows::FairyGuiWindow>,
}
impl GuiObjectResolver<'_, '_> {
    /// Resolve declarative configuration to its current, materialized view.
    /// Copied GObjectRef components cannot declare policies for another view.
    pub(crate) fn view(&self, runtime: &GuiRuntime, entity: Entity) -> Option<(GObjectId, Entity)> {
        let object = self.object(runtime, entity)?;
        let view = runtime.entity_of(object)?;
        (self.references.get(entity).is_err() || entity == view).then_some((object, view))
    }

    pub(crate) fn object(&self, runtime: &GuiRuntime, entity: Entity) -> Option<GObjectId> {
        resolve_object(
            runtime,
            entity,
            self.references.get(entity).ok(),
            self.roots.get(entity).ok(),
            self.windows.get(entity).ok(),
        )
    }
}

/// Direct access to materialized GUI objects. Calls apply immediately; pending
/// assets return `None`. Use writable ECS components for declarative properties.
/// Root/window aliases require their current template to match the materialized
/// view. Use ParamSet with mutable FairyGuiRoot/FairyGuiWindow queries, since
/// this service reads those components to validate aliases at the time of use.
#[derive(SystemParam, Debug)]
pub struct GuiMut<'w, 's> {
    runtime: NonSendMut<'w, GuiRuntime>,
    resolver: GuiObjectResolver<'w, 's>,
}
impl GuiMut<'_, '_> {
    fn object_for_entity(&self, entity: Entity) -> Option<GObjectId> {
        self.resolver.object(&self.runtime, entity)
    }
    pub fn get_mut(&mut self, entity: Entity) -> Option<GuiObjectMut<'_>> {
        let object = self.object_for_entity(entity)?;
        self.runtime.store().object(object)?;
        Some(GuiObjectMut {
            runtime: &mut self.runtime,
            object,
        })
    }
    pub fn edit(&mut self, entity: Entity, edit: impl FnOnce(&mut GuiObjectMut<'_>)) -> bool {
        if let Some(mut object) = self.get_mut(entity) {
            edit(&mut object);
            true
        } else {
            false
        }
    }

    /// Capture a custom notification's logical bubbling path now and publish it
    /// as GuiCustomEvent at the next GuiSystems::Publish. Accepts ready views or
    /// current root/window owners. Returns false for unavailable/stale entities;
    /// no retry is queued. Dispatch from a Publish-stage observer publishes on
    /// the next frame.
    pub fn dispatch_custom(
        &mut self,
        entity: Entity,
        name: impl Into<String>,
        data: crate::GuiEventData,
    ) -> bool {
        let Some(object) = self.object_for_entity(entity) else {
            return false;
        };
        dispatch_custom(&mut self.runtime, object, name.into(), data);
        true
    }

    /// Reveal a parented target using current logical geometry. Direct children
    /// use their untransformed bounds; nested targets convert the parent's two
    /// rectangle corners, matching native ScrollPane. Accepts ready view/root/
    /// window entities. Returns whether the scroll target or animation changed.
    /// Missing targets, singular transforms and non-scroll containers return false.
    /// For unmaterialized virtual rows, use scroll_list_item_to_view instead.
    pub fn scroll_object_to_view(
        &mut self,
        container: Entity,
        target: Entity,
        animated: bool,
        set_first: bool,
    ) -> bool {
        let Some((container, target)) = self
            .object_for_entity(container)
            .zip(self.object_for_entity(target))
        else {
            return false;
        };
        self.runtime
            .store_mut()
            .scroll_object_to_view(container, target, animated, set_first)
    }

    /// Swap two ready siblings using native sorting restrictions. Both view
    /// entities and current root/window owners are accepted. Applies immediately;
    /// Sync publishes the logical and paint order without rebuilding views.
    /// Returns whether order changed; stale, pending or unrelated targets fail.
    pub fn swap_children(&mut self, first: Entity, second: Entity) -> bool {
        let Some((first, second)) = self
            .object_for_entity(first)
            .zip(self.object_for_entity(second))
        else {
            return false;
        };
        self.runtime.store_mut().swap_children(first, second)
    }

    /// Join a ready sibling GGroup, or leave with None. Applies membership
    /// immediately; Sync refreshes layout, display and hierarchy snapshots.
    /// Returns whether changed. Unready/non-group/cross-parent targets and
    /// membership cycles are rejected. Repeating an assignment is a no-op.
    pub fn set_group(&mut self, member: Entity, group: Option<Entity>) -> bool {
        let Some(member) = self.object_for_entity(member) else {
            return false;
        };
        let group = match group {
            Some(entity) => {
                let Some(group) = self.object_for_entity(entity) else {
                    return false;
                };
                Some(group)
            }
            None => None,
        };
        set_group(&mut self.runtime, member, group)
    }

    /// Retain a relation between ready view/root/window entities. Preserves the
    /// current offset and applies immediately. Duplicate definitions keep their
    /// original percent policy. Missing, pending or self targets return false.
    pub fn add_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
        percent: bool,
    ) -> bool {
        let Some((owner, target)) = self
            .object_for_entity(owner)
            .zip(self.object_for_entity(target))
        else {
            return false;
        };
        self.runtime
            .store_mut()
            .add_relation(owner, target, relation, percent)
    }

    /// Remove one type; Size removes both Width and Height. Returns whether changed.
    pub fn remove_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool {
        let Some((owner, target)) = self
            .object_for_entity(owner)
            .zip(self.object_for_entity(target))
        else {
            return false;
        };
        self.runtime
            .store_mut()
            .remove_relation(owner, target, relation)
    }

    /// Check a retained type. Size requires both Width and Height.
    pub fn has_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool {
        let Some((owner, target)) = self
            .object_for_entity(owner)
            .zip(self.object_for_entity(target))
        else {
            return false;
        };
        self.runtime
            .store_mut()
            .has_relation(owner, target, relation)
    }

    /// Clear every definition referring to one ready entity.
    pub fn clear_relations_to(&mut self, owner: Entity, target: Entity) -> bool {
        let Some((owner, target)) = self
            .object_for_entity(owner)
            .zip(self.object_for_entity(target))
        else {
            return false;
        };
        self.runtime.store_mut().clear_relations_to(owner, target)
    }

    /// Clear exported and runtime relations on one ready entity.
    pub fn clear_relations(&mut self, owner: Entity) -> bool {
        let Some(owner) = self.object_for_entity(owner) else {
            return false;
        };
        self.runtime.store_mut().clear_relations(owner)
    }
}
/// An immediate mutable view of one native object.
pub struct GuiObjectMut<'a> {
    runtime: &'a mut GuiRuntime,
    object: GObjectId,
}
impl std::fmt::Debug for GuiObjectMut<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiObjectMut")
            .field("object", &self.object)
            .finish()
    }
}
impl GuiObjectMut<'_> {
    /// Execute native GButton.fireClick. Check/Radio selection changes enqueue
    /// GuiStateChanged; no GuiClick or pointer press is synthesized and linked
    /// popups stay untouched. Common buttons optionally show 100/200 ms down/up
    /// feedback. Hidden/noninteractive buttons still execute native behavior.
    /// Returns false for a non-button; uses current model settings immediately.
    pub fn fire_click(&mut self, down_effect: bool) -> bool {
        self.runtime.fire_button_click(self.object, down_effect)
    }

    /// Unsubscribe this object from all exported and runtime relation targets.
    pub fn clear_relations(&mut self) -> bool {
        self.runtime.store_mut().clear_relations(self.object)
    }
    /// Raise a ready native window without sending a lifecycle message.
    pub fn bring_window_to_front(&mut self) -> bool {
        self.runtime
            .window_index(self.object)
            .is_some_and(|index| self.runtime.bring_window_to_front(index))
    }

    /// Center a ready window in the current FairyGUI viewport. Viewport size is
    /// refreshed from GuiInputSettings' chosen Bevy window during synchronization.
    pub fn center_window(&mut self) -> bool {
        let Some(index) = self.runtime.window_index(self.object) else {
            return false;
        };
        let Some(root) = self.runtime.root.as_ref() else {
            return false;
        };
        self.runtime.windows[index].center_on(&mut self.runtime.store, root);
        true
    }
    pub fn set_tree_settings(&mut self, settings: crate::GuiTreeSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiTreeSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_list_settings(&mut self, settings: crate::GuiListSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiListSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_group_layout(&mut self, layout: crate::GuiGroupLayout) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiGroupLayout::read(self.runtime, self.object) {
            layout.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_list_layout(&mut self, layout: crate::GuiListLayout) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiListLayout::read(self.runtime, self.object) {
            layout.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_scroll_settings(&mut self, settings: crate::GuiScrollSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiScrollSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_opaque(&mut self, opaque: bool) {
        self.runtime
            .store_mut()
            .set_component_opaque(self.object, opaque);
    }

    pub fn set_clip_softness(&mut self, softness: Vec2) {
        self.runtime.store_mut().set_clip_softness(
            self.object,
            fairygui_core::Point::new(softness.x, softness.y),
        );
    }
    pub fn set_image_flip(&mut self, flip: crate::FlipType) {
        self.runtime.store_mut().set_image_flip(self.object, flip);
    }
    pub fn set_blend_mode(&mut self, mode: Option<crate::BlendMode>) {
        use crate::properties::Property;
        let value = crate::GuiBlendMode(mode);
        if let Some(native) = crate::GuiBlendMode::read(self.runtime, self.object) {
            value.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_color_filter(&mut self, filter: Option<crate::ColorFilterParams>) {
        use crate::properties::Property;
        let value = crate::GuiColorFilter(filter);
        if let Some(native) = crate::GuiColorFilter::read(self.runtime, self.object) {
            value.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_graph(&mut self, graph: crate::GuiGraph) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiGraph::read(self.runtime, self.object) {
            graph.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_button_settings(&mut self, settings: crate::GuiButtonSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiButtonSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_input_settings(&mut self, settings: crate::GuiTextInputSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiTextInputSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_color(&mut self, color: Color) {
        use crate::properties::Property;
        let value = crate::GuiColor(color);
        if let Some(native) = crate::GuiColor::read(self.runtime, self.object) {
            value.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_image_fill(&mut self, fill: crate::GuiImageFill) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiImageFill::read(self.runtime, self.object) {
            fill.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_loader_settings(&mut self, settings: crate::GuiLoaderSettings) {
        use crate::properties::Property;
        if let Some(native) = crate::GuiLoaderSettings::read(self.runtime, self.object) {
            settings.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn text_format(&self) -> Option<crate::GuiTextFormat> {
        <crate::GuiTextFormat as crate::properties::Property>::read(self.runtime, self.object)
    }

    pub fn set_text_format(&mut self, format: crate::GuiTextFormat) {
        use crate::properties::Property;
        if let Some(native) = self.text_format() {
            format.apply(&native, &native, self.runtime, self.object);
        }
    }
    pub fn set_text_variables(&mut self, variables: crate::GuiTextVars) {
        self.runtime
            .store_mut()
            .set_object_text_variables(self.object, variables.0);
    }
    pub fn set_html_options(&mut self, options: crate::GuiHtmlOptions) {
        self.runtime
            .store_mut()
            .set_object_html_options(self.object, options.into());
    }
    pub fn set_children_render_order(&mut self, order: crate::ChildrenRenderOrder) {
        self.runtime
            .store_mut()
            .set_children_render_order(self.object, order);
    }
    pub fn set_apex_index(&mut self, index: i32) {
        self.runtime.store_mut().set_apex_index(self.object, index);
    }
    pub fn sorting_order(&self) -> i32 {
        self.runtime
            .store()
            .object(self.object)
            .unwrap()
            .sorting_order()
    }
    pub fn set_sorting_order(&mut self, order: i32) {
        self.runtime
            .store_mut()
            .set_sorting_order(self.object, order);
    }
    pub fn child_index(&self) -> Option<usize> {
        self.runtime.store().child_index(self.object)
    }
    /// Move this ordinary child, clamping before siblings with nonzero priority.
    pub fn set_child_index(&mut self, index: usize) -> Option<usize> {
        self.runtime.store_mut().set_child_index(self.object, index)
    }
    /// The index is measured before removing this child from its current slot.
    pub fn set_child_index_before(&mut self, index: usize) -> Option<usize> {
        self.runtime
            .store_mut()
            .set_child_index_before(self.object, index)
    }
    pub fn text(&self) -> Option<&str> {
        self.runtime.store().object_text(self.object)
    }
    pub fn set_animation_playing(&mut self, playing: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_animation_playing(object, playing);
    }
    pub fn set_animation_frame(&mut self, frame: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_animation_frame(object, frame);
    }
    pub fn set_animation_time_scale(&mut self, scale: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_animation_time_scale(object, scale);
    }
    pub fn set_animation_play_settings(&mut self, settings: crate::GuiAnimationPlaySettings) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_animation_play_settings(object, settings);
    }
    pub fn advance_animation(&mut self, seconds: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().advance_animation(object, seconds);
    }
    pub fn rewind_animation(&mut self) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().rewind_animation(object);
    }
    pub fn tween(&mut self, target: crate::GuiTweenTarget, duration: f32, ease: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let target = &target;
        target.apply(runtime.store_mut(), object, duration, ease);
    }
    pub fn stop_tween(&mut self, property: crate::GuiTweenProperty) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().kill_tween_property(object, property)
    }
    pub fn set_position(&mut self, position: Vec2) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if position.is_finite() {
            runtime.store_mut().set_xy(object, position.x, position.y);
        }
    }
    pub fn set_size_constraints(&mut self, constraints: crate::GuiSizeConstraints) {
        use crate::properties::Property;
        let object = self.object;
        let runtime = &mut *self.runtime;
        if let Some(native) = crate::GuiSizeConstraints::read(runtime, object) {
            constraints.apply(&native, &native, runtime, object);
        }
    }
    pub fn set_width(&mut self, width: f32) {
        if width.is_finite() && width >= 0. {
            let runtime = &mut *self.runtime;
            let object = self.object;
            runtime.store_mut().set_width(object, width);
        }
    }
    pub fn set_height(&mut self, height: f32) {
        if height.is_finite() && height >= 0. {
            let runtime = &mut *self.runtime;
            let object = self.object;
            runtime.store_mut().set_height(object, height);
        }
    }
    pub fn set_size(&mut self, size: Vec2) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if size.is_finite() && size.min_element() >= 0.0 {
            runtime.store_mut().set_size(object, size.x, size.y);
        }
    }
    pub fn set_visible(&mut self, visible: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_visible(object, visible);
    }
    pub fn set_touchable(&mut self, touchable: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_touchable(object, touchable);
    }
    pub fn set_alpha(&mut self, alpha: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if alpha.is_finite() {
            runtime.store_mut().set_alpha(object, alpha);
        }
    }
    pub fn set_scale(&mut self, scale: Vec2) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if scale.is_finite() {
            runtime.store_mut().set_scale(object, scale.x, scale.y);
        }
    }
    pub fn set_pivot(&mut self, pivot: Vec2, as_anchor: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if pivot.is_finite() {
            runtime
                .store_mut()
                .set_pivot(object, pivot.x, pivot.y, as_anchor);
        }
    }
    pub fn set_rotation(&mut self, rotation: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if rotation.is_finite() {
            runtime.store_mut().set_rotation(object, rotation);
        }
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let text = text.into();
        let text = &text;
        runtime.store_mut().set_object_text(object, text);
    }
    pub fn set_combo_items(&mut self, items: Vec<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_combo_items(object, items);
    }
    pub fn set_combo_values(&mut self, values: Vec<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_combo_values(object, values);
    }
    pub fn set_combo_icons(&mut self, icons: Option<Vec<Option<String>>>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_combo_icons(object, icons);
    }
    pub fn set_combo_selection(&mut self, index: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().select_combo_index(object, index);
    }
    pub fn set_combo_value(&mut self, value: Option<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .select_combo_value(object, value.as_deref());
    }
    pub fn set_combo_visible_item_count(&mut self, count: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_combo_visible_item_count(object, count);
    }
    pub fn set_combo_popup_direction(&mut self, direction: crate::GuiPopupDirection) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let code = match direction {
            crate::GuiPopupDirection::Auto => 0,
            crate::GuiPopupDirection::Up => 1,
            crate::GuiPopupDirection::Down => 2,
        };
        runtime.store_mut().set_combo_popup_direction(object, code);
    }
    pub fn set_combo_selection_controller(&mut self, name: Option<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = &name;
        runtime
            .store_mut()
            .set_combo_selection_controller(object, name.as_deref());
    }
    pub fn set_tooltips(&mut self, text: Option<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_tooltips(object, text);
    }
    pub fn set_data(&mut self, data: Option<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_data(object, data);
    }
    pub fn ensure_text_size(&mut self) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let measurer = runtime.text_measurer.clone();
        measurer.borrow_mut().ensure(&mut runtime.store, object);
    }
    pub fn set_icon(&mut self, icon: impl Into<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let icon = icon.into();
        let icon = &icon;
        runtime.store_mut().apply_icon_to(object, icon);
    }
    pub fn set_grayed(&mut self, grayed: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_grayed(object, grayed);
    }
    pub fn set_button_selected(&mut self, selected: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_button_selected(object, selected);
    }
    pub fn set_button_change_state_on_click(&mut self, enabled: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_button_change_state_on_click(object, enabled);
    }
    pub fn set_slider_value(&mut self, value: f64) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_slider_value(object, value);
    }
    pub fn set_slider_range(&mut self, min: f64, max: f64) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_slider_range(object, min, max);
    }
    pub fn set_slider_whole_numbers(&mut self, whole: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_slider_whole_numbers(object, whole);
    }
    pub fn set_slider_change_on_click(&mut self, change: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_slider_change_on_click(object, change);
    }
    pub fn set_slider_can_drag(&mut self, can_drag: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_slider_can_drag(object, can_drag);
    }
    pub fn set_slider_title_type(&mut self, title_type: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_slider_title_type(object, title_type);
    }
    pub fn set_controller_page(&mut self, name: impl Into<String>, index: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        runtime
            .store_mut()
            .set_controller_page_index(object, name, index);
    }
    pub fn set_progress(&mut self, value: f64) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_progress(object, value);
    }
    pub fn set_progress_range(&mut self, min: f64, max: f64) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_progress_range(object, min, max);
    }
    pub fn set_progress_title_type(&mut self, title_type: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_progress_title_type(object, title_type);
    }
    pub fn tween_progress(&mut self, value: f64, duration: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().tween_progress(object, value, duration);
    }
    pub fn stop_progress_tween(&mut self) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().stop_progress_tween(object);
    }
    pub fn set_list_selection(&mut self, index: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.store_mut().set_list_selection(object, index);
    }
    /// Explicit native arrow navigation, starting at the first selected row.
    /// Replaces selection and reveals the destination without emitting a click.
    /// Bind keyboard/gamepad input in your own ECS system. Returns whether the
    /// selected indices changed; unavailable rows and boundaries are ignored.
    pub fn navigate_list(&mut self, direction: ListNavigationDirection) -> bool {
        self.runtime
            .store_mut()
            .navigate_list_selection(self.object, direction)
    }
    pub fn set_tree_expanded(&mut self, index: usize, expanded: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .set_tree_node_expanded(object, index, expanded);
    }
    pub fn toggle_tree_node(&mut self, index: usize) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.toggle_tree_node(object, index);
    }
    pub fn expand_tree(&mut self) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.expand_tree(object);
    }
    pub fn collapse_tree(&mut self) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime.collapse_tree(object);
    }
    pub fn select_tree_node(&mut self, index: usize, scroll_to_view: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let valid = matches!(runtime.store().object(object).map(|o|o.control()),Some(ControlData::Tree(data)) if index<data.node_count());
        if valid {
            runtime.expand_tree_to(object, index);
            if let Some(visible) = runtime.store().tree_visible_index_of_node(object, index) {
                runtime
                    .store_mut()
                    .add_list_selection_with_scroll(object, visible, scroll_to_view);
            }
        }
    }
    /// Select a node by its current visible GTree child index.
    pub fn select_tree_visible_node(&mut self, visible_index: usize, scroll_to_view: bool) -> bool {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let Some(index) = runtime
            .store()
            .tree_node_at_visible_index(object, visible_index)
        else {
            return false;
        };
        runtime.expand_tree_to(object, index);
        runtime
            .store()
            .tree_visible_index_of_node(object, index)
            .is_some_and(|visible| {
                runtime
                    .store_mut()
                    .add_list_selection_with_scroll(object, visible, scroll_to_view)
            })
    }
    /// Lock a scroll header in logical pixels; zero releases it. Call in Respond
    /// to set the boundary before the first release animation tick. Returns
    /// whether the lock changed; invalid sizes and non-scroll objects are ignored.
    pub fn lock_scroll_header(&mut self, size: f32) -> bool {
        self.runtime
            .store_mut()
            .lock_scroll_header(self.object, size)
    }
    /// Lock a scroll footer in logical pixels; zero releases it. Like header
    /// locking, this applies immediately and reports whether the lock changed.
    pub fn lock_scroll_footer(&mut self, size: f32) -> bool {
        self.runtime
            .store_mut()
            .lock_scroll_footer(self.object, size)
    }
    pub fn set_scroll_position(&mut self, position: Vec2, allow_bounce: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if position.is_finite() {
            runtime
                .store_mut()
                .set_scroll_pos(object, position.x, position.y, allow_bounce);
        }
    }
    /// Scroll a logical list item into view without changing its selection.
    pub fn scroll_list_item_to_view(&mut self, index: usize, animated: bool) {
        self.runtime
            .store_mut()
            .scroll_list_item_to_view(self.object, index, animated, false);
    }
    /// Native resizeToFit: fit height for SingleColumn/FlowHorizontal, width
    /// otherwise. None uses the native 100000-item limit. Ordinary lists apply
    /// min_size; virtual lists use template dimensions and ignore that minimum.
    /// Measures ready text before fitting and preserves margins/bar gutters.
    /// Returns whether the effective size changed; Sync projects ECS geometry.
    pub fn resize_list_to_fit(&mut self, item_count: Option<usize>, min_size: f32) -> bool {
        if !min_size.is_finite()
            || min_size < 0.
            || self
                .runtime
                .store()
                .list_layout_settings(self.object)
                .is_none()
        {
            return false;
        }
        self.runtime
            .text_measurer
            .borrow_mut()
            .ensure_subtree(&mut self.runtime.store, self.object);
        self.runtime
            .store_mut()
            .resize_list_to_fit(self.object, item_count, min_size)
    }
    /// Native getMaxItemWidth: maximum unscaled materialized row width,
    /// including hidden rows. Returns None for a non-list object.
    pub fn max_list_item_width(&self) -> Option<f32> {
        self.runtime.store().list_max_item_width(self.object)
    }
    /// Read a cached virtual-row rectangle in logical content coordinates.
    /// Returns None for non-virtual lists or indices outside the logical count.
    pub fn virtual_item_rect(&self, index: usize) -> Option<fairygui_core::Rect> {
        self.runtime.store().virtual_item_rect(self.object, index)
    }
    /// Native virtualItemSize baseline, independent of measured row dimensions.
    pub fn virtual_item_size(&self) -> Option<Vec2> {
        self.runtime
            .store()
            .virtual_item_size(self.object)
            .map(|size| Vec2::new(size.width, size.height))
    }
    /// Change an initialized virtual list's baseline. Existing cached row sizes
    /// and the pane's scroll step remain intact. Sync refreshes visible rows.
    pub fn set_virtual_item_size(
        &mut self,
        size: Vec2,
    ) -> Result<(), fairygui_core::object::VirtualListError> {
        self.runtime
            .store_mut()
            .set_virtual_item_size(self.object, fairygui_core::Size::new(size.x, size.y))
    }
    pub fn select_list_index(&mut self, index: usize, scroll_to_view: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        runtime
            .store_mut()
            .add_list_selection_with_scroll(object, index, scroll_to_view);
    }
    pub fn scroll_to(&mut self, position: Vec2, animated: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if position.is_finite() {
            runtime.store_mut().layout_scroll_bars(object);
            runtime
                .store_mut()
                .set_scroll_pos_animated(object, position.x, position.y, animated);
        }
    }
    pub fn set_scroll_percentage(&mut self, percentage: Vec2, animated: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        if percentage.is_finite() {
            runtime.store_mut().layout_scroll_bars(object);
            if let Some(pane) = runtime.store().scroll_pane(object) {
                let max = pane.max_scroll_pos();
                let position = percentage.clamp(Vec2::ZERO, Vec2::ONE) * Vec2::new(max.x, max.y);
                runtime
                    .store_mut()
                    .set_scroll_pos_animated(object, position.x, position.y, animated);
            }
        }
    }
    pub fn scroll_to_view(
        &mut self,
        rect: fairygui_core::geom::Rect,
        animated: bool,
        set_first: bool,
    ) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let rect = &rect;
        runtime
            .store_mut()
            .scroll_rect_to_view(object, *rect, animated, set_first);
    }
    pub fn play_transition(&mut self, name: impl Into<String>, times: i32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        runtime.store_mut().play_transition(object, name, times);
    }
    pub fn play_transition_with_options(
        &mut self,
        name: impl Into<String>,
        options: fairygui_core::transition::TransitionPlayOptions,
    ) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        let options = &options;
        runtime
            .store_mut()
            .play_transition_with_options(object, name, *options);
    }
    pub fn set_transition_paused(&mut self, name: impl Into<String>, paused: bool) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        runtime
            .store_mut()
            .set_transition_paused(object, name, paused);
    }
    pub fn set_transition_time_scale(&mut self, name: impl Into<String>, scale: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        if scale.is_finite() {
            runtime
                .store_mut()
                .set_transition_time_scale(object, name, scale);
        }
    }
    pub fn play_transition_reverse(&mut self, name: impl Into<String>, delay: f32) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        if delay.is_finite() {
            runtime
                .store_mut()
                .play_transition_reverse(object, name, delay);
        }
    }
    pub fn stop_transition(&mut self, name: impl Into<String>) {
        let runtime = &mut *self.runtime;
        let object = self.object;
        let name = name.into();
        let name = &name;
        runtime.store_mut().stop_transition(object, name);
    }
}
/// Direct object access for exclusive systems and tests.
pub trait GuiWorldExt {
    fn gui_mut(&mut self, entity: Entity) -> Option<GuiObjectMut<'_>>;
    /// Equivalent of GuiMut::scroll_object_to_view for exclusive systems.
    fn gui_scroll_object_to_view(
        &mut self,
        container: Entity,
        target: Entity,
        animated: bool,
        set_first: bool,
    ) -> bool;
    /// Equivalent of GuiMut::dispatch_custom for exclusive systems.
    fn gui_dispatch_custom(
        &mut self,
        entity: Entity,
        name: impl Into<String>,
        data: crate::GuiEventData,
    ) -> bool;
    /// Immediate equivalent of GuiMut::swap_children for exclusive systems.
    fn gui_swap_children(&mut self, first: Entity, second: Entity) -> bool;
    /// Immediate equivalent of GuiMut::set_group for exclusive systems.
    fn gui_set_group(&mut self, member: Entity, group: Option<Entity>) -> bool;
    /// Immediate equivalent of GuiMut::add_relation for exclusive systems.
    fn gui_add_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
        percent: bool,
    ) -> bool;
    fn gui_remove_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool;
    fn gui_has_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool;
    fn gui_clear_relations_to(&mut self, owner: Entity, target: Entity) -> bool;
}

fn dispatch_custom(
    runtime: &mut GuiRuntime,
    object: GObjectId,
    name: String,
    data: crate::GuiEventData,
) {
    runtime.dispatch_event(
        fairygui_core::object::UiEvent::new(fairygui_core::object::EventKind::Other(name), object)
            .with_data(data),
    );
}

fn set_group(runtime: &mut GuiRuntime, member: GObjectId, group: Option<GObjectId>) -> bool {
    let Some(object) = runtime.store().object(member) else {
        return false;
    };
    if object.group() == group {
        return false;
    }
    if let Some(group) = group {
        let Some(target) = runtime.store().object(group) else {
            return false;
        };
        if target.kind() != ObjectType::Group
            || object.parent().is_none()
            || object.parent() != target.parent()
        {
            return false;
        }
    }
    runtime.store_mut().set_group(member, group);
    runtime.store().object(member).unwrap().group() == group
}

fn world_object(world: &World, entity: Entity) -> Option<GObjectId> {
    resolve_object(
        world.get_non_send::<GuiRuntime>()?,
        entity,
        world.get::<GObjectRef>(entity),
        world.get::<FairyGuiRoot>(entity),
        world.get::<windows::FairyGuiWindow>(entity),
    )
}
impl GuiWorldExt for World {
    fn gui_scroll_object_to_view(
        &mut self,
        container: Entity,
        target: Entity,
        animated: bool,
        set_first: bool,
    ) -> bool {
        let Some((container, target)) =
            world_object(self, container).zip(world_object(self, target))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| {
                runtime
                    .store_mut()
                    .scroll_object_to_view(container, target, animated, set_first)
            })
    }
    fn gui_dispatch_custom(
        &mut self,
        entity: Entity,
        name: impl Into<String>,
        data: crate::GuiEventData,
    ) -> bool {
        let Some(object) = world_object(self, entity) else {
            return false;
        };
        let Some(mut runtime) = self.get_non_send_mut::<GuiRuntime>() else {
            return false;
        };
        dispatch_custom(&mut runtime, object, name.into(), data);
        true
    }

    fn gui_swap_children(&mut self, first: Entity, second: Entity) -> bool {
        let Some((first, second)) = world_object(self, first).zip(world_object(self, second))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| runtime.store_mut().swap_children(first, second))
    }

    fn gui_set_group(&mut self, member: Entity, group: Option<Entity>) -> bool {
        let Some(member) = world_object(self, member) else {
            return false;
        };
        let group = match group {
            Some(entity) => {
                let Some(group) = world_object(self, entity) else {
                    return false;
                };
                Some(group)
            }
            None => None,
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| set_group(&mut runtime, member, group))
    }

    fn gui_mut(&mut self, entity: Entity) -> Option<GuiObjectMut<'_>> {
        let object = world_object(self, entity)?;
        let runtime = self.get_non_send_mut::<GuiRuntime>()?.into_inner();
        runtime.store().object(object)?;
        Some(GuiObjectMut { runtime, object })
    }
    fn gui_add_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
        percent: bool,
    ) -> bool {
        let Some((owner, target)) = world_object(self, owner).zip(world_object(self, target))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| {
                runtime
                    .store_mut()
                    .add_relation(owner, target, relation, percent)
            })
    }
    fn gui_remove_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool {
        let Some((owner, target)) = world_object(self, owner).zip(world_object(self, target))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| runtime.store_mut().remove_relation(owner, target, relation))
    }
    fn gui_has_relation(
        &mut self,
        owner: Entity,
        target: Entity,
        relation: crate::RelationType,
    ) -> bool {
        let Some((owner, target)) = world_object(self, owner).zip(world_object(self, target))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| runtime.store_mut().has_relation(owner, target, relation))
    }
    fn gui_clear_relations_to(&mut self, owner: Entity, target: Entity) -> bool {
        let Some((owner, target)) = world_object(self, owner).zip(world_object(self, target))
        else {
            return false;
        };
        self.get_non_send_mut::<GuiRuntime>()
            .is_some_and(|mut runtime| runtime.store_mut().clear_relations_to(owner, target))
    }
}
impl GuiRuntime {
    /// Edit an already materialized object by its view or owner entity.
    pub fn edit(&mut self, entity: Entity, edit: impl FnOnce(&mut GuiObjectMut<'_>)) -> bool {
        let object = self
            .entities
            .iter()
            .find_map(|(&object, &view)| (view == entity).then_some(object))
            .or_else(|| self.ecs_root_object(entity))
            .or_else(|| self.ecs_window_object(entity));
        let Some(object) = object.filter(|&object| self.store().contains(object)) else {
            return false;
        };
        edit(&mut GuiObjectMut {
            runtime: self,
            object,
        });
        true
    }
}
