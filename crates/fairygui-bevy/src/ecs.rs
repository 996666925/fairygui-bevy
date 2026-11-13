//! ECS queries for FairyGUI roots and logical control hierarchies.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use fairygui_core::object::GObject;
use fairygui_core::package::ObjectType;

use crate::GuiWindowState;
use crate::list::GuiListState;
use crate::runtime::{GObjectRef, GuiRootInstance, GuiRuntime};

#[cfg(test)]
#[path = "ecs_scroll_tests.rs"]
mod scroll_tests;
#[cfg(test)]
#[path = "ecs_space_tests.rs"]
mod space_tests;

#[cfg(test)]
#[path = "ecs_transition_tests.rs"]
mod transition_tests;

/// Read-only state of one exported transition, with time and duration in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct GuiTransitionState {
    name: String,
    playing: bool,
    paused: bool,
    reversed: bool,
    time: f32,
    duration: f32,
    time_scale: f32,
    play_generation: u64,
}

impl GuiTransitionState {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }
    pub fn time(&self) -> f32 {
        self.time
    }
    /// Exported nominal duration; repeated and nested tasks can finish later.
    pub fn duration(&self) -> f32 {
        self.duration
    }
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }
    pub fn play_generation(&self) -> u64 {
        self.play_generation
    }

    fn matches(&self, source: &fairygui_core::transition::Transition) -> bool {
        self.name == source.name
            && self.playing == source.is_playing()
            && self.paused == source.is_paused()
            && self.reversed == source.is_reversed()
            && self.time == source.time()
            && self.duration == source.total_duration
            && self.time_scale == source.time_scale()
            && self.play_generation == source.play_generation()
    }

    fn from_transition(source: &fairygui_core::transition::Transition) -> Self {
        Self {
            name: source.name.clone(),
            playing: source.is_playing(),
            paused: source.is_paused(),
            reversed: source.is_reversed(),
            time: source.time(),
            duration: source.total_duration,
            time_scale: source.time_scale(),
            play_generation: source.play_generation(),
        }
    }
}

/// Exported transitions in package order. Refreshed before Publish and after
/// Sync; absent on objects without transitions. Mutate through GUI commands.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiTransitions(Vec<GuiTransitionState>);

impl GuiTransitions {
    pub fn states(&self) -> &[GuiTransitionState] {
        &self.0
    }
    pub fn get(&self, name: &str) -> Option<&GuiTransitionState> {
        self.0.iter().find(|state| state.name == name)
    }
}

/// Rendered root entity belonging to a [`crate::FairyGuiRoot`] owner.
/// Available after `GuiSystems::Sync`; removed when the root is replaced.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiRootView(pub Entity);

/// Native refresh overlay. Sizes use logical pixels; source size is the
/// exported size, independent of the current pull extent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuiPullPart {
    pub entity: Entity,
    pub size: Vec2,
    pub source_size: Vec2,
}

/// Read-only scroll-pane state in logical pixels, refreshed before input events
/// publish and after Sync. Absent on controls without a scroll pane.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiScrollState {
    position: Vec2,
    display_position: Vec2,
    view_size: Vec2,
    content_size: Vec2,
    max_position: Vec2,
    percentage: Vec2,
    display_percentage: Vec2,
    current_page: IVec2,
    animating: bool,
    dragging: bool,
    header: Option<GuiPullPart>,
    footer: Option<GuiPullPart>,
    refresh_vertical: Option<bool>,
    header_locked_size: f32,
    footer_locked_size: f32,
}

impl GuiScrollState {
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
    /// Logical endpoint requested by an animated scroll.
    pub fn position(&self) -> Vec2 {
        self.position
    }

    /// Current offset used for rendering and hit testing during animation.
    pub fn display_position(&self) -> Vec2 {
        self.display_position
    }

    pub fn view_size(&self) -> Vec2 {
        self.view_size
    }

    pub fn content_size(&self) -> Vec2 {
        self.content_size
    }

    pub fn max_position(&self) -> Vec2 {
        self.max_position
    }

    /// Clamped 0..=1 logical progress; zero on an axis without overflow.
    pub fn percentage(&self) -> Vec2 {
        self.percentage
    }

    pub fn display_percentage(&self) -> Vec2 {
        self.display_percentage
    }

    pub fn current_page(&self) -> IVec2 {
        self.current_page
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }
    pub fn header(&self) -> Option<&GuiPullPart> {
        self.header.as_ref()
    }
    pub fn footer(&self) -> Option<&GuiPullPart> {
        self.footer.as_ref()
    }
    /// True for a vertical refresh axis, false for horizontal; absent without refresh resources.
    pub fn refresh_vertical(&self) -> Option<bool> {
        self.refresh_vertical
    }
    pub fn header_locked_size(&self) -> f32 {
        self.header_locked_size
    }
    pub fn footer_locked_size(&self) -> f32 {
        self.footer_locked_size
    }
}

/// Read-only controller pages in package order. Updated before input events
/// publish, before PostLayout, and at the end of Sync.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiControllers(Vec<GuiControllerState>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiControllerState {
    name: String,
    page_ids: Vec<String>,
    page_names: Vec<String>,
    selected_index: i32,
    previous_index: i32,
}
impl GuiControllerState {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn page_ids(&self) -> &[String] {
        &self.page_ids
    }
    pub fn page_names(&self) -> &[String] {
        &self.page_names
    }
    pub fn selected_index(&self) -> i32 {
        self.selected_index
    }
    pub fn previous_index(&self) -> i32 {
        self.previous_index
    }
    pub fn selected_page_id(&self) -> Option<&str> {
        usize::try_from(self.selected_index)
            .ok()
            .and_then(|index| self.page_ids.get(index))
            .map(String::as_str)
    }
    pub fn selected_page_name(&self) -> Option<&str> {
        usize::try_from(self.selected_index)
            .ok()
            .and_then(|index| self.page_names.get(index))
            .map(String::as_str)
    }
}
impl GuiControllers {
    pub fn states(&self) -> &[GuiControllerState] {
        &self.0
    }
    pub fn get(&self, name: &str) -> Option<&GuiControllerState> {
        self.0.iter().find(|state| state.name == name)
    }
}

/// Read-only native text property, before translation, markup and masking.
///
/// Buttons expose their base title; labels and combo boxes forward to the title
/// child. Inputs expose committed text, excluding prompts and IME preedit.
/// Absent on objects without a text property. Write via `GuiObjectCommand`.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiText(String);

impl GuiText {
    pub fn value(&self) -> &str {
        &self.0
    }
}

/// Native icon URL of a Loader, button, label or combo box. Empty means no icon;
/// buttons expose the base icon even while a selected icon is displayed.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiIcon(String);
impl GuiIcon {
    pub fn value(&self) -> &str {
        &self.0
    }
}

/// Read-only local drawing and interaction state from the FairyGUI model.
///
/// Explicit visibility, controller/group display visibility and stage attachment
/// are independent. `display_visible` excludes ancestor visibility and clipping;
/// it does not guarantee a painted pixel or an eligible input target.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiDisplay {
    visible: bool,
    display_visible: bool,
    on_stage: bool,
    alpha: f32,
    effective_alpha: f32,
    grayed: bool,
    touchable: bool,
}

impl GuiDisplay {
    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn display_visible(&self) -> bool {
        self.display_visible
    }

    pub fn on_stage(&self) -> bool {
        self.on_stage
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Product of this object's alpha and all logical ancestors' alpha.
    pub fn effective_alpha(&self) -> f32 {
        self.effective_alpha
    }

    pub fn grayed(&self) -> bool {
        self.grayed
    }

    pub fn touchable(&self) -> bool {
        self.touchable
    }
}

/// Read-only FairyGUI geometry, refreshed after `GuiSystems::Sync`.
///
/// Position is relative to the logical parent, in logical pixels, and size is
/// unscaled. Rotation and skew use degrees. These are model values, before
/// Bevy layout, scroll offsets, pivot compensation and inherited transforms.
/// Change geometry through [`crate::GuiObjectCommand`].
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiTransform {
    position: Vec2,
    size: Vec2,
    scale: Vec2,
    skew: Vec2,
    pivot: Vec2,
    pivot_as_anchor: bool,
    rotation: f32,
}

/// Native logical local/global conversion snapshot, including anchor pivots,
/// ancestor transforms, margins, scroll display offsets and virtual alignment.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiSpace(fairygui_core::object::CoordinateTransform);
impl GuiSpace {
    pub fn local_to_global(&self, point: Vec2) -> Option<Vec2> {
        let point = self
            .0
            .transform_point(fairygui_core::Point::new(point.x, point.y))?;
        Some(Vec2::new(point.x, point.y))
    }
    pub fn global_to_local(&self, point: Vec2) -> Option<Vec2> {
        let point = self
            .0
            .inverse()?
            .transform_point(fairygui_core::Point::new(point.x, point.y))?;
        Some(Vec2::new(point.x, point.y))
    }
    pub fn local_to_global_rect(&self, rect: fairygui_core::Rect) -> Option<fairygui_core::Rect> {
        let low = self.local_to_global(Vec2::new(rect.x, rect.y))?;
        let high = self.local_to_global(Vec2::new(rect.right(), rect.bottom()))?;
        Some(fairygui_core::Rect::new(
            low.x,
            low.y,
            high.x - low.x,
            high.y - low.y,
        ))
    }
    pub fn global_to_local_rect(&self, rect: fairygui_core::Rect) -> Option<fairygui_core::Rect> {
        let low = self.global_to_local(Vec2::new(rect.x, rect.y))?;
        let high = self.global_to_local(Vec2::new(rect.right(), rect.bottom()))?;
        Some(fairygui_core::Rect::new(
            low.x,
            low.y,
            high.x - low.x,
            high.y - low.y,
        ))
    }
}

impl GuiTransform {
    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn scale(&self) -> Vec2 {
        self.scale
    }

    pub fn skew(&self) -> Vec2 {
        self.skew
    }

    /// Pivot as a fraction of the unscaled size.
    pub fn pivot(&self) -> Vec2 {
        self.pivot
    }

    pub fn pivot_as_anchor(&self) -> bool {
        self.pivot_as_anchor
    }

    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    fn from_object(object: &GObject) -> Self {
        let pivot = object.pivot();
        Self {
            position: Vec2::new(object.x(), object.y()),
            size: Vec2::new(object.width(), object.height()),
            scale: Vec2::new(object.scale_x(), object.scale_y()),
            skew: Vec2::new(object.skew_x(), object.skew_y()),
            pivot: Vec2::new(pivot.x, pivot.y),
            pivot_as_anchor: object.pivot_as_anchor(),
            rotation: object.rotation(),
        }
    }
}

/// Read-only identity and logical hierarchy of a rendered FairyGUI control.
/// Decorative Bevy entities are excluded, and children retain FairyGUI order.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiObjectInfo {
    name: String,
    kind: ObjectType,
    parent: Option<Entity>,
    children: Vec<Entity>,
    children_in_view: Option<Vec<Entity>>,
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

    /// Direct children intersecting this component's native viewport. This is
    /// a geometry query, independent of visibility, scaling or ancestor clips.
    pub fn children_in_view(&self) -> Option<&[Entity]> {
        self.children_in_view.as_deref()
    }
}

/// Read FairyGUI identities and find control entities using only ECS queries.
///
/// Every method accepts either a root owner or a control view entity. Hierarchy
/// and geometry in `GuiSystems::Respond` read the preceding Sync snapshot.
/// Text, display, scroll and transition state also refresh before input events
/// publish.
/// Systems after `GuiSystems::Sync` can bind controls created this frame.
#[derive(SystemParam, Debug)]
pub struct GuiQuery<'w, 's> {
    roots: Query<'w, 's, &'static GuiRootView>,
    objects: Query<'w, 's, &'static GuiObjectInfo>,
    transforms: Query<'w, 's, &'static GuiTransform>,
    spaces: Query<'w, 's, &'static GuiSpace>,
    texts: Query<'w, 's, &'static GuiText>,
    icons: Query<'w, 's, &'static GuiIcon>,
    loaders: Query<'w, 's, &'static crate::GuiLoaderState>,
    displays: Query<'w, 's, &'static GuiDisplay>,
    scrolls: Query<'w, 's, &'static GuiScrollState>,
    transitions: Query<'w, 's, &'static GuiTransitions>,
    lists: Query<'w, 's, &'static GuiListState>,
    controllers: Query<'w, 's, &'static GuiControllers>,
    windows: Query<'w, 's, &'static GuiWindowState>,
    trees: Query<'w, 's, &'static crate::GuiTreeState>,
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
    pub fn window(&self, entity: Entity) -> Option<&GuiWindowState> {
        self.windows
            .get(entity)
            .ok()
            .or_else(|| self.windows.get(self.view(entity)?).ok())
    }

    pub fn get(&self, entity: Entity) -> Option<&GuiObjectInfo> {
        self.objects.get(self.view(entity)?).ok()
    }

    /// Logical geometry of a root owner or control view entity.
    pub fn transform(&self, entity: Entity) -> Option<&GuiTransform> {
        self.transforms.get(self.view(entity)?).ok()
    }

    pub fn space(&self, entity: Entity) -> Option<&GuiSpace> {
        self.spaces.get(self.view(entity)?).ok()
    }
    pub fn local_to_global(&self, entity: Entity, point: Vec2) -> Option<Vec2> {
        self.space(entity)?.local_to_global(point)
    }
    pub fn global_to_local(&self, entity: Entity, point: Vec2) -> Option<Vec2> {
        self.space(entity)?.global_to_local(point)
    }
    pub fn local_to_global_rect(
        &self,
        entity: Entity,
        rect: fairygui_core::Rect,
    ) -> Option<fairygui_core::Rect> {
        self.space(entity)?.local_to_global_rect(rect)
    }
    pub fn global_to_local_rect(
        &self,
        entity: Entity,
        rect: fairygui_core::Rect,
    ) -> Option<fairygui_core::Rect> {
        self.space(entity)?.global_to_local_rect(rect)
    }

    /// Native text of a root owner or control view, before presentation.
    pub fn text(&self, entity: Entity) -> Option<&str> {
        Some(self.texts.get(self.view(entity)?).ok()?.value())
    }

    pub fn icon(&self, entity: Entity) -> Option<&str> {
        Some(self.icons.get(self.view(entity)?).ok()?.value())
    }

    pub fn loader(&self, entity: Entity) -> Option<&crate::GuiLoaderState> {
        self.loaders.get(self.view(entity)?).ok()
    }

    pub fn display(&self, entity: Entity) -> Option<&GuiDisplay> {
        self.displays.get(self.view(entity)?).ok()
    }

    pub fn scroll(&self, entity: Entity) -> Option<&GuiScrollState> {
        self.scrolls.get(self.view(entity)?).ok()
    }

    pub fn list(&self, entity: Entity) -> Option<&GuiListState> {
        self.lists.get(self.view(entity)?).ok()
    }
    pub fn tree(&self, entity: Entity) -> Option<&crate::GuiTreeState> {
        self.trees.get(self.view(entity)?).ok()
    }
    pub fn controllers(&self, entity: Entity) -> Option<&GuiControllers> {
        self.controllers.get(self.view(entity)?).ok()
    }
    pub fn controller(&self, entity: Entity, name: &str) -> Option<&GuiControllerState> {
        self.controllers(entity)?.get(name)
    }

    pub fn transitions(&self, entity: Entity) -> Option<&GuiTransitions> {
        self.transitions.get(self.view(entity)?).ok()
    }

    pub fn transition(&self, entity: Entity, name: &str) -> Option<&GuiTransitionState> {
        self.transitions(entity)?.get(name)
    }

    /// First direct child with the exact FairyGUI object name.
    pub fn child_by_name(&self, parent: Entity, name: &str) -> Option<Entity> {
        self.get(parent)?
            .children
            .iter()
            .copied()
            .find(|&child| self.objects.get(child).is_ok_and(|info| info.name == name))
    }

    /// Native inclusive-edge geometry test for a direct logical child. Owners
    /// resolve to views; missing/non-component/unrelated targets return None.
    /// Like hierarchy/geometry snapshots, refreshes before PostLayout and at Sync end.
    pub fn is_child_in_view(&self, parent: Entity, child: Entity) -> Option<bool> {
        let parent = self.view(parent)?;
        let child = self.view(child)?;
        if self.get(child)?.parent() != Some(parent) {
            return None;
        }
        let info = self.get(parent)?;
        Some(info.children_in_view()?.contains(&child))
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

/// Refresh before publishing input events and after all model/view updates.
pub(crate) fn sync_gui_transitions(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(Entity, &GObjectRef, Option<&GuiTransitions>)>,
) {
    for (entity, reference, current) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let transitions = runtime
            .store()
            .object(reference.0)
            .and_then(|object| object.control().component())
            .map_or(&[][..], |component| component.transitions.as_slice());
        if transitions.is_empty() {
            if current.is_some() {
                commands.entity(entity).remove::<GuiTransitions>();
            }
            continue;
        }
        if current.is_none_or(|current| {
            current.0.len() != transitions.len()
                || !current
                    .0
                    .iter()
                    .zip(transitions)
                    .all(|(state, transition)| state.matches(transition))
        }) {
            commands.entity(entity).insert(GuiTransitions(
                transitions
                    .iter()
                    .map(GuiTransitionState::from_transition)
                    .collect(),
            ));
        }
    }
}

pub(crate) fn sync_gui_scroll_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(Entity, &GObjectRef, Option<&GuiScrollState>)>,
) {
    for (entity, reference, current) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(pane) = runtime.store().scroll_pane(reference.0) else {
            if current.is_some() {
                commands.entity(entity).remove::<GuiScrollState>();
            }
            continue;
        };
        let point = |p: fairygui_core::geom::Point| Vec2::new(p.x, p.y);
        let size = |s: fairygui_core::geom::Size| Vec2::new(s.width, s.height);
        let part = |id: Option<fairygui_core::object::GObjectId>| {
            let id = id?;
            let object = runtime.store().object(id)?;
            Some(GuiPullPart {
                entity: runtime.entity_of(id)?,
                size: size(object.size()),
                source_size: size(object.source_size()),
            })
        };
        let next = GuiScrollState {
            position: point(pane.pos()),
            display_position: point(pane.display_pos()),
            view_size: size(pane.view_size()),
            content_size: size(pane.content_size()),
            max_position: point(pane.max_scroll_pos()),
            percentage: Vec2::new(pane.perc_x(), pane.perc_y()),
            display_percentage: Vec2::new(pane.display_perc_x(), pane.display_perc_y()),
            current_page: IVec2::new(pane.current_page_x(), pane.current_page_y()),
            animating: pane.is_animating(),
            dragging: pane.is_dragging(),
            header: part(runtime.store().scroll_header(reference.0)),
            footer: part(runtime.store().scroll_footer(reference.0)),
            refresh_vertical: pane.refresh_axis(),
            header_locked_size: pane.header_locked_size(),
            footer_locked_size: pane.footer_locked_size(),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

pub(crate) fn sync_gui_icons(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(Entity, &GObjectRef, Option<&GuiIcon>)>,
) {
    use fairygui_core::widget::ControlData;
    for (entity, reference, current) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let source =
            runtime
                .store()
                .object(reference.0)
                .and_then(|object| match object.control() {
                    ControlData::Loader(data) => Some(data.url.as_deref().unwrap_or_default()),
                    ControlData::Button(data) => Some(data.icon.as_deref().unwrap_or_default()),
                    ControlData::Label(data) => Some(data.icon.as_deref().unwrap_or_default()),
                    ControlData::ComboBox(data) => Some(data.icon.as_deref().unwrap_or_default()),
                    _ => None,
                });
        match (current, source) {
            (current, Some(source)) if current.is_none_or(|current| current.value() != source) => {
                commands.entity(entity).insert(GuiIcon(source.to_owned()));
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<GuiIcon>();
            }
            _ => {}
        }
    }
}

pub(crate) fn sync_gui_properties(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(Entity, &GObjectRef, Option<&GuiText>, Option<&GuiDisplay>)>,
) {
    let store = runtime.store();
    for (entity, reference, text, display) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(object) = store.object(reference.0) else {
            continue;
        };
        match (text, store.object_text(reference.0)) {
            (current, Some(value)) if current.is_none_or(|current| current.value() != value) => {
                commands.entity(entity).insert(GuiText(value.to_owned()));
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<GuiText>();
            }
            _ => {}
        }
        let next = GuiDisplay {
            visible: object.visible(),
            display_visible: store.display_visible(reference.0),
            on_stage: store.is_on_stage(reference.0),
            alpha: object.alpha(),
            effective_alpha: store.effective_alpha(reference.0),
            grayed: object.grayed(),
            touchable: object.touchable(),
        };
        if display != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_gui_objects(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut objects: Query<(
        Entity,
        &GObjectRef,
        Option<&mut GuiObjectInfo>,
        Option<&GuiTransform>,
        Option<&GuiSpace>,
    )>,
    roots: Query<(Entity, &GuiRootInstance, Option<&GuiRootView>)>,
) {
    for (entity, reference, current, transform, space) in &mut objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(object) = runtime.store().object(reference.0) else {
            continue;
        };
        let next_transform = GuiTransform::from_object(object);
        if let Some(next) = runtime
            .store()
            .coordinate_transform(reference.0)
            .map(GuiSpace)
            && space != Some(&next)
        {
            commands.entity(entity).insert(next);
        }
        if transform != Some(&next_transform) {
            commands.entity(entity).insert(next_transform);
        }
        let parent = object.parent().and_then(|id| runtime.entity_of(id));
        let children = object
            .children()
            .iter()
            .filter_map(|&id| runtime.entity_of(id));
        let children_in_view = object.control().component().map(|_| {
            object
                .children()
                .iter()
                .copied()
                .filter(|&child| runtime.store().is_child_in_view(reference.0, child) == Some(true))
                .filter_map(|child| runtime.entity_of(child))
                .collect::<Vec<_>>()
        });
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
            if current.children_in_view != children_in_view {
                current.children_in_view = children_in_view;
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
                children_in_view,
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

pub(crate) fn sync_gui_controllers(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(Entity, &GObjectRef, Option<&GuiControllers>)>,
) {
    for (entity, reference, current) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let source = runtime
            .store()
            .object(reference.0)
            .and_then(|object| object.control().component())
            .map_or(&[][..], |component| component.controllers.as_slice());
        if source.is_empty() {
            if current.is_some() {
                commands.entity(entity).remove::<GuiControllers>();
            }
            continue;
        }
        if current.is_some_and(|current| {
            current.0.len() == source.len()
                && current.0.iter().zip(source).all(|(state, controller)| {
                    state.name == controller.name
                        && state.page_ids == controller.page_ids
                        && state.page_names == controller.page_names
                        && state.selected_index == controller.selected_index
                        && state.previous_index == controller.previous_index
                })
        }) {
            continue;
        }
        commands.entity(entity).insert(GuiControllers(
            source
                .iter()
                .map(|controller| GuiControllerState {
                    name: controller.name.clone(),
                    page_ids: controller.page_ids.clone(),
                    page_names: controller.page_names.clone(),
                    selected_index: controller.selected_index,
                    previous_index: controller.previous_index,
                })
                .collect(),
        ));
    }
}
