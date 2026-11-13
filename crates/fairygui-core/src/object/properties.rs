use super::*;
use crate::color::Color;
use crate::widget::ControlData;

impl ObjectStore {
    /// Native text property, including forwarded label and combo-box titles.
    /// Buttons return their base title even while a selected title is displayed.
    /// Input fields return committed text, before translation and presentation.
    pub fn object_text(&self, mut id: GObjectId) -> Option<&str> {
        loop {
            match self.object(id)?.control() {
                ControlData::Text(data) => return Some(&data.text),
                ControlData::TextInput(data) => return Some(&data.text_field.text),
                ControlData::Button(data) => return Some(&data.title),
                ControlData::Label(_) | ControlData::ComboBox(_) => {
                    id = self.child_by_name(id, "title")?;
                }
                _ => return None,
            }
        }
    }

    /// Native GTextField.stroke, forwarded to compound titles. Width changes do
    /// not capture Color or FontSize gears; strokeColor has its separate setter.
    pub fn set_object_stroke(&mut self, id: GObjectId, width: f32) -> bool {
        if !width.is_finite() {
            return false;
        }
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(data) => data,
            ControlData::TextInput(data) => &mut data.text_field,
            _ => return false,
        };
        if field.stroke == width {
            return false;
        }
        field.stroke = width;
        self.touch(target, dirty::TEXT);
        true
    }

    /// Resolve an input itself or the input title of a compound control.
    pub fn input_field_of(&self, id: GObjectId) -> Option<GObjectId> {
        self.text_field_of(id)
            .filter(|&target| matches!(self.objects[target].control, ControlData::TextInput(_)))
    }

    /// Mask input presentation without changing its value or saved text gears.
    pub fn set_input_password(&mut self, id: GObjectId, password: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return false;
        };
        if data.password == password {
            return false;
        }
        data.password = password;
        self.touch(target, dirty::TEXT);
        true
    }

    /// Set raw GTextInput prompt text. UBB removal belongs to prompt projection.
    pub fn set_input_prompt(&mut self, id: GObjectId, prompt: Option<&str>) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return false;
        };
        if data.prompt_text.as_deref() == prompt {
            return false;
        }
        data.prompt_text = prompt.map(str::to_owned);
        self.touch(target, dirty::TEXT);
        true
    }

    pub fn set_input_prompt_color(&mut self, id: GObjectId, color: Color) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return false;
        };
        if data.prompt_color == color {
            return false;
        }
        data.prompt_color = color;
        self.touch(target, dirty::COLOR);
        true
    }

    /// Read-only inputs retain selection, copy and programmatic text assignment.
    pub fn set_input_editable(&mut self, id: GObjectId, editable: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return false;
        };
        if data.editable == editable {
            return false;
        }
        data.editable = editable;
        self.touch(target, dirty::TEXT);
        true
    }
    /// Set the user-edit length limit; zero/negative select the platform default.
    /// Existing/programmatically assigned text is not truncated.
    pub fn set_input_max_length(&mut self, id: GObjectId, max: i32) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return false;
        };
        if data.max_length == max {
            return false;
        }
        data.max_length = max;
        self.touch(target, dirty::TEXT);
        true
    }

    /// Validate native restriction syntax before installing it. Compound controls
    /// forward to an input title. Programmatic text assignments bypass restriction.
    pub fn set_input_restrict(
        &mut self,
        id: GObjectId,
        restrict: Option<&str>,
    ) -> Result<bool, regress::Error> {
        let Some(target) = self.text_field_of(id) else {
            return Ok(false);
        };
        let ControlData::TextInput(data) = &mut self.objects[target].control else {
            return Ok(false);
        };
        if data.restrict.as_deref() == restrict {
            return Ok(false);
        }
        crate::text::input::InputRestriction::new(restrict)?;
        data.restrict = restrict.map(str::to_owned);
        self.touch(target, dirty::TEXT);
        Ok(true)
    }

    /// GTextInput.singleLine also selects the native multiline editing mode.
    pub fn set_object_single_line(&mut self, id: GObjectId, single: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(d) => d,
            ControlData::TextInput(d) => &mut d.text_field,
            _ => return false,
        };
        if field.single_line == single {
            return false;
        }
        field.single_line = single;
        self.touch(target, dirty::TEXT);
        true
    }
    /// Set GTextField.underline, forwarding compound controls to their title.
    pub fn set_object_underline(&mut self, id: GObjectId, enabled: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(d) => d,
            ControlData::TextInput(d) => &mut d.text_field,
            _ => return false,
        };
        if field.underline == enabled {
            return false;
        }
        field.underline = enabled;
        self.touch(target, dirty::TEXT);
        true
    }
    /// Native Text.html mode, independent of the UBB-to-HTML conversion flag.
    pub fn set_object_html_enabled(&mut self, id: GObjectId, enabled: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(d) => d,
            ControlData::TextInput(d) => &mut d.text_field,
            _ => return false,
        };
        if field.html_enabled == enabled {
            return false;
        }
        field.html_enabled = enabled;
        self.touch(target, dirty::TEXT);
        true
    }
    /// Select a default/system or package bitmap font. Like GTextField.font,
    /// changing it invalidates text without updating the font-size gear.
    /// Compound controls forward to their resolved title text field.
    pub fn set_object_font(&mut self, id: GObjectId, font: Option<&str>) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(data) => data,
            ControlData::TextInput(data) => &mut data.text_field,
            _ => return false,
        };
        if field.font.as_deref() == font {
            return false;
        }
        field.font = font.map(str::to_owned);
        self.touch(target, dirty::TEXT);
        true
    }

    /// Enable/disable UBB parsing without replacing the object or its view.
    pub fn set_object_ubb_enabled(&mut self, id: GObjectId, enabled: bool) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(data) => data,
            ControlData::TextInput(data) => &mut data.text_field,
            _ => return false,
        };
        if field.ubb_enabled == enabled {
            return false;
        }
        field.ubb_enabled = enabled;
        self.touch(target, dirty::TEXT);
        true
    }

    /// Configure native Image/GLoader fill masking. Layout, pivot and pointer
    /// rectangles remain unchanged; only the drawn image/movie content is masked.
    pub fn set_image_fill(
        &mut self,
        id: GObjectId,
        method: u8,
        origin: u8,
        clockwise: bool,
        amount: impl Into<f64>,
    ) -> bool {
        let amount = amount.into();
        if !amount.is_finite() {
            return false;
        }
        let Some(control) = self.objects.get_mut(id).map(|o| &mut o.control) else {
            return false;
        };
        let (m, o, c, a) = match control {
            ControlData::Image(d) => (
                &mut d.fill_method,
                &mut d.fill_origin,
                &mut d.fill_clockwise,
                &mut d.fill_amount,
            ),
            ControlData::Loader(d) => (
                &mut d.fill_method,
                &mut d.fill_origin,
                &mut d.fill_clockwise,
                &mut d.fill_amount,
            ),
            _ => return false,
        };
        if (*m, *o, *c, *a) == (method, origin, clockwise, amount) {
            return false;
        }
        (*m, *o, *c, *a) = (method, origin, clockwise, amount);
        self.touch(id, dirty::IMAGE);
        true
    }

    pub fn set_image_fill_amount(&mut self, id: GObjectId, amount: impl Into<f64>) -> bool {
        let Some((method, origin, cw)) = self.object(id).and_then(|o| match o.control() {
            ControlData::Image(d) => Some((d.fill_method, d.fill_origin, d.fill_clockwise)),
            ControlData::Loader(d) => Some((d.fill_method, d.fill_origin, d.fill_clockwise)),
            _ => None,
        }) else {
            return false;
        };
        self.set_image_fill(id, method, origin, cw, amount)
    }

    /// Replace a graph's drawing command, as native drawPolygon/drawRegularPolygon
    /// do. Geometry is rebuilt by the renderer without replacing the object/view.
    pub fn set_graph(&mut self, id: GObjectId, graph: crate::widget::GraphData) -> bool {
        let Some(ControlData::Graph(current)) = self.objects.get_mut(id).map(|o| &mut o.control)
        else {
            return false;
        };
        if *current == graph {
            return false;
        }
        *current = graph;
        self.touch(id, dirty::LOOK);
        true
    }

    /// Native showErrorSign flag. Changing it affects subsequent failures; it
    /// neither removes an already displayed error sign nor triggers a retry.
    pub fn set_loader_show_error_sign(&mut self, loader: GObjectId, show: bool) -> bool {
        let Some(ControlData::Loader(data)) = self
            .objects
            .get_mut(loader)
            .map(|object| &mut object.control)
        else {
            return false;
        };
        if data.show_error_sign == show {
            return false;
        }
        data.show_error_sign = show;
        true
    }

    /// Native GLoader.useResize. Components resize their logical canvas instead
    /// of scaling it; images and movies continue to use their content rectangle.
    pub fn set_loader_use_resize(&mut self, id: GObjectId, use_resize: bool) -> bool {
        let Some(ControlData::Loader(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return false;
        };
        if data.use_resize == use_resize {
            return false;
        }
        data.use_resize = use_resize;
        self.touch(id, dirty::IMAGE);
        true
    }

    /// Layout an owned component using the package item's original dimensions.
    /// As in native GLoader, switching modes does not reset the other mode's
    /// previously written size/scale. Auto-size uses 50x30 for zero dimensions.
    pub fn layout_loader_component(
        &mut self,
        loader: GObjectId,
        component: GObjectId,
        source: crate::geom::Size,
    ) -> bool {
        let Some(object) = self.object(loader) else {
            return false;
        };
        let ControlData::Loader(data) = object.control() else {
            return false;
        };
        let data = data.clone();
        let Some(content) = self.object(component) else {
            return false;
        };
        if content.control().component().is_none() || content.parent() != Some(loader) {
            return false;
        }
        let source = if data.auto_size {
            crate::geom::Size::new(
                if source.width > 0.0 {
                    source.width
                } else {
                    50.0
                },
                if source.height > 0.0 {
                    source.height
                } else {
                    30.0
                },
            )
        } else {
            source
        };
        if data.auto_size {
            self.set_size(loader, source.width, source.height);
        }
        let rect = data.content_rect(source, self.object(loader).unwrap().size());
        if data.use_resize {
            self.set_size(component, rect.width, rect.height);
        } else {
            self.set_scale(
                component,
                if source.width > 0.0 {
                    rect.width / source.width
                } else {
                    1.0
                },
                if source.height > 0.0 {
                    rect.height / source.height
                } else {
                    1.0
                },
            );
        }
        self.set_xy(component, rect.x, rect.y);
        true
    }

    pub fn set_combo_hovered(&mut self, id: GObjectId, hovered: bool) {
        let Some(ControlData::ComboBox(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return;
        };
        data.hovered = hovered;
        if data.pressed || self.combo_popups.contains_key(&id) {
            return;
        }
        self.set_controller_page(id, "button", if hovered { "over" } else { "up" });
    }

    pub fn set_combo_pressed(&mut self, id: GObjectId, pressed: bool) {
        let Some(ControlData::ComboBox(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return;
        };
        data.pressed = pressed;
        let hovered = data.hovered;
        if !pressed && !self.combo_popups.contains_key(&id) {
            self.set_controller_page(id, "button", if hovered { "over" } else { "up" });
        }
    }

    /// 序列帧的播放速度；不影响 Playing/Frame 或已累积的帧内时间。
    pub fn animation_time_scale(&self, id: GObjectId) -> Option<f32> {
        match &self.object(id)?.control {
            ControlData::MovieClip(data) => Some(data.time_scale.unwrap_or(1.0)),
            ControlData::Loader(data) => Some(data.time_scale.unwrap_or(1.0)),
            _ => None,
        }
    }

    pub fn set_animation_time_scale(&mut self, id: GObjectId, scale: f32) -> bool {
        if !scale.is_finite() || scale < 0.0 {
            return false;
        }
        let value = match self.objects.get_mut(id).map(|o| &mut o.control) {
            Some(ControlData::MovieClip(data)) => &mut data.time_scale,
            Some(ControlData::Loader(data)) => &mut data.time_scale,
            _ => return false,
        };
        if value.unwrap_or(1.0) == scale {
            return false;
        }
        *value = Some(scale);
        true
    }
    /// Configure image/movie loader sizing without changing its resource URL.
    pub fn set_loader_layout(&mut self, id: GObjectId, fill: i32, align: i32, vertical_align: i32) {
        let Some(ControlData::Loader(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return;
        };
        if (data.fill, data.align, data.vertical_align) != (fill, align, vertical_align) {
            (data.fill, data.align, data.vertical_align) = (fill, align, vertical_align);
            self.touch(id, dirty::IMAGE);
        }
    }

    pub fn set_loader_auto_size(&mut self, id: GObjectId, auto_size: bool) {
        let Some(ControlData::Loader(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return;
        };
        if data.auto_size != auto_size {
            data.auto_size = auto_size;
            self.touch(id, dirty::IMAGE);
        }
    }

    pub fn set_loader_shrink_only(&mut self, id: GObjectId, shrink_only: bool) {
        let Some(ControlData::Loader(data)) =
            self.objects.get_mut(id).map(|object| &mut object.control)
        else {
            return;
        };
        if data.shrink_only != shrink_only {
            data.shrink_only = shrink_only;
            self.touch(id, dirty::IMAGE);
        }
    }

    /// Movie clips and loaders share FairyGUI's Playing/Frame properties.
    pub fn animation_state(&self, id: GObjectId) -> Option<(bool, i32)> {
        match self.objects.get(id)?.control() {
            ControlData::MovieClip(data) => Some((data.playing, data.frame)),
            ControlData::Loader(data) => Some((data.playing, data.frame)),
            _ => None,
        }
    }

    pub fn set_animation_playing(&mut self, id: GObjectId, playing: bool) {
        let Some(object) = self.objects.get_mut(id) else {
            return;
        };
        let value = match &mut object.control {
            ControlData::MovieClip(data) => &mut data.playing,
            ControlData::Loader(data) => &mut data.playing,
            _ => return,
        };
        if *value != playing {
            *value = playing;
            self.touch(id, dirty::IMAGE);
            self.update_gear_state(id, crate::widget::GearKind::Animation);
        }
    }

    pub fn set_animation_frame(&mut self, id: GObjectId, frame: i32) {
        let changed = self
            .animation_state(id)
            .is_some_and(|(_, current)| current != frame);
        if self.animation_state(id).is_some_and(|(_, current)| {
            current != frame
                || self
                    .animation_commands
                    .get(&id)
                    .is_some_and(|commands| !commands.is_empty())
        }) {
            self.animation_commands
                .entry(id)
                .or_default()
                .push(AnimationCommand::SetFrame(frame));
            self.report_animation_frame(id, frame);
            self.touch(id, dirty::IMAGE);
            if changed {
                self.update_gear_state(id, crate::widget::GearKind::Animation);
            }
        }
    }

    /// 排队执行原版 DeltaTime 属性；0 也会清除已累积的帧内余量。
    pub fn advance_animation(&mut self, id: GObjectId, seconds: f32) -> bool {
        if !seconds.is_finite() || seconds < 0.0 {
            return false;
        }
        let Some((_, frame)) = self.animation_state(id) else {
            return false;
        };
        let commands = self.animation_commands.entry(id).or_default();
        // A source created after these commands sees the final model frame.
        // Retain the starting frame so earlier advances run from the right state.
        if commands.is_empty() {
            commands.push(AnimationCommand::SetFrame(frame));
        }
        commands.push(AnimationCommand::Advance(seconds));
        true
    }

    pub fn take_animation_commands(&mut self, id: GObjectId) -> Vec<AnimationCommand> {
        self.animation_commands.remove(&id).unwrap_or_default()
    }

    /// 配置分段播放并定位到起始帧；不改变 Playing 或速度。
    pub fn set_animation_play_settings(
        &mut self,
        id: GObjectId,
        settings: AnimationPlaySettings,
    ) -> bool {
        if !settings.is_valid() || self.animation_state(id).is_none() {
            return false;
        }
        self.completed_animations.retain(|owner| *owner != id);
        self.animation_commands
            .entry(id)
            .or_default()
            .push(AnimationCommand::SetPlaySettings(settings));
        self.report_animation_frame(id, settings.start);
        self.touch(id, dirty::IMAGE);
        true
    }

    /// 回到第 0 帧，清除帧内时间、方向和循环计数；不改变分段播放配置。
    pub fn rewind_animation(&mut self, id: GObjectId) -> bool {
        if self.animation_state(id).is_none() {
            return false;
        }
        self.animation_commands
            .entry(id)
            .or_default()
            .push(AnimationCommand::Rewind);
        self.report_animation_frame(id, 0);
        self.touch(id, dirty::IMAGE);
        true
    }

    /// 适配器在有限次数播放结束时回报一次，Playing 属性仍保留原值。
    pub fn report_animation_complete(&mut self, id: GObjectId) {
        if self.animation_state(id).is_some() {
            self.completed_animations.push(id);
        }
    }

    pub fn take_animation_completions(&mut self) -> Vec<GObjectId> {
        std::mem::take(&mut self.completed_animations)
    }

    /// Playback adapters report the current frame without requesting another
    /// seek or a view rebuild. Commands must use set_animation_frame instead.
    pub fn report_animation_frame(&mut self, id: GObjectId, frame: i32) {
        match self.objects.get_mut(id).map(|object| &mut object.control) {
            Some(ControlData::MovieClip(data)) => data.frame = frame,
            Some(ControlData::Loader(data)) => data.frame = frame,
            _ => {}
        }
    }

    /// Apply the Color/OutlineColor properties supported by the target control.
    pub fn set_control_color(&mut self, id: GObjectId, color: Color, outline: Option<Color>) {
        let Some(object) = self.objects.get_mut(id) else {
            return;
        };
        let capture = match &mut object.control {
            ControlData::Image(data) => {
                let changed = data.color.unwrap_or(Color::WHITE) != color;
                data.color = Some(color);
                changed
            }
            ControlData::MovieClip(data) => {
                data.color = Some(color);
                false
            }
            ControlData::Loader(data) => {
                let changed = data.color.unwrap_or(Color::WHITE) != color;
                data.color = Some(color);
                changed
            }
            ControlData::Graph(data) => {
                data.fill_color = color;
                true
            }
            ControlData::Text(data) => {
                let changed = data.color != color;
                data.color = color;
                changed
            }
            ControlData::TextInput(data) => {
                let changed = data.text_field.color != color;
                data.text_field.color = color;
                changed
            }
            ControlData::Button(data) => {
                data.title_color = Some(color);
                if let Some(title) = self.text_field_of(id) {
                    self.set_control_color(title, color, None);
                }
                self.update_gear_state(id, crate::widget::GearKind::Color);
                if let Some(outline) = outline {
                    self.set_control_outline_color(id, outline);
                }
                return;
            }
            ControlData::Label(data) => {
                data.title_color = Some(color);
                if let Some(title) = self.text_field_of(id) {
                    self.set_control_color(title, color, None);
                }
                self.update_gear_state(id, crate::widget::GearKind::Color);
                if let Some(outline) = outline {
                    self.set_control_outline_color(id, outline);
                }
                return;
            }
            ControlData::ComboBox(data) => {
                data.title_color = Some(color);
                if let Some(title) = self.text_field_of(id) {
                    self.set_control_color(title, color, None);
                }
                self.update_gear_state(id, crate::widget::GearKind::Color);
                if let Some(outline) = outline {
                    self.set_control_outline_color(id, outline);
                }
                return;
            }
            _ => return,
        };
        self.touch(id, dirty::COLOR);
        if capture {
            self.update_gear_state(id, crate::widget::GearKind::Color);
        }
        if let Some(outline) = outline {
            self.set_control_outline_color(id, outline);
        }
    }

    /// Native OutlineColor forwards to the text field and updates its Color
    /// gear; forwarding compound controls do not capture their own gear here.
    pub fn set_control_outline_color(&mut self, id: GObjectId, color: Color) {
        let Some(target) = self.text_field_of(id) else {
            return;
        };
        let field = match &mut self.objects[target].control {
            ControlData::Text(d) => d,
            ControlData::TextInput(d) => &mut d.text_field,
            _ => return,
        };
        let changed = field
            .stroke_color
            .unwrap_or(Color::from_rgba8(0, 0, 0, 255))
            != color;
        field.stroke_color = Some(color);
        self.touch(target, dirty::COLOR);
        if changed {
            self.update_gear_state(target, crate::widget::GearKind::Color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_text_getters_forward_titles_but_keep_button_base_values() {
        let mut store = ObjectStore::new();
        let button = store.create(ObjectType::Button);
        let button_title = store.create(ObjectType::Text);
        store.set_name(button_title, "title");
        store.set_parent(button_title, Some(button));
        store.set_object_text(button, "Base");
        if let ControlData::Button(data) = &mut store.object_mut(button).unwrap().control {
            data.selected = true;
            data.selected_title = Some("Selected".into());
        }
        store.apply_title_of(button);
        assert_eq!(store.object_text(button), Some("Base"));
        assert_eq!(store.object_text(button_title), Some("Selected"));

        for kind in [ObjectType::Label, ObjectType::ComboBox] {
            let owner = store.create(kind);
            assert_eq!(store.object_text(owner), None);
            let title = store.create(ObjectType::Label);
            store.set_name(title, "title");
            store.set_parent(title, Some(owner));
            let input = store.create(ObjectType::InputText);
            store.set_name(input, "title");
            store.set_parent(input, Some(title));
            store.set_input_password(input, true);
            store.set_input_prompt(input, Some("Prompt"));
            store.set_object_text(owner, "真实😀[b]raw[/b]");
            assert_eq!(store.object_text(owner), Some("真实😀[b]raw[/b]"));
            store.set_object_text(input, "Direct edit");
            assert_eq!(store.object_text(owner), Some("Direct edit"));
            store.remove(input);
            assert_eq!(store.object_text(owner), None);
        }
        let image = store.create(ObjectType::Image);
        assert_eq!(store.object_text(image), None);
        store.remove(button);
        assert_eq!(store.object_text(button), None);
    }
}
