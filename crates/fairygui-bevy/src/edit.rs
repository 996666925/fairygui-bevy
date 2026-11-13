//! Direct object editing for behavior such as playback and scrolling.
use super::*;
use bevy::ecs::system::SystemParam;

/// Direct access to materialized GUI objects. Calls apply immediately; pending
/// assets return `None`. Use writable ECS components for declarative properties.
#[derive(SystemParam, Debug)]
pub struct GuiMut<'w, 's> {
    runtime: NonSendMut<'w, GuiRuntime>,
    references: Query<'w, 's, &'static GObjectRef>,
}
impl GuiMut<'_, '_> {
    pub fn get_mut(&mut self, entity: Entity) -> Option<GuiObjectMut<'_>> {
        let object = self
            .references
            .get(entity)
            .ok()
            .map(|r| r.0)
            .or_else(|| self.runtime.ecs_root_object(entity))
            .or_else(|| self.runtime.ecs_window_object(entity))?;
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
    /// Swap two materialized siblings using native sorting restrictions.
    /// Returns false for unrelated, missing or unchanged objects.
    pub fn swap_with(&mut self, other: Entity) -> bool {
        let other = self
            .runtime
            .entities
            .iter()
            .find_map(|(&object, &entity)| (entity == other).then_some(object))
            .or_else(|| self.runtime.ecs_root_object(other))
            .or_else(|| self.runtime.ecs_window_object(other));
        other.is_some_and(|other| self.runtime.store_mut().swap_children(self.object, other))
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
            runtime
                .store_mut()
                .add_list_selection_with_scroll(object, index, scroll_to_view);
        }
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
}
impl GuiWorldExt for World {
    fn gui_mut(&mut self, entity: Entity) -> Option<GuiObjectMut<'_>> {
        let object = self
            .get::<GObjectRef>(entity)
            .map(|r| r.0)
            .or_else(|| self.get::<GuiRootInstance>(entity).map(|r| r.object))
            .or_else(|| self.get_non_send::<GuiRuntime>()?.ecs_root_object(entity))
            .or_else(|| self.get_non_send::<GuiRuntime>()?.ecs_window_object(entity))?;
        let runtime = self.get_non_send_mut::<GuiRuntime>()?.into_inner();
        runtime.store().object(object)?;
        Some(GuiObjectMut { runtime, object })
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
