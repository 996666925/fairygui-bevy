use super::{ObjectStore, dirty};
use crate::{object::GObjectId, widget::ControlData};

impl ObjectStore {
    /// GButton handles its disabled pages before GComponent's controller/child
    /// branches. Leaf handlers only change their local appearance in the adapter.
    pub(super) fn handle_grayed_changed(&mut self, id: GObjectId) {
        let Some(object) = self.object(id) else {
            return;
        };
        if matches!(object.control(),ControlData::Button(data) if data.component.controller("button").is_some_and(|c|c.has_page("disabled")))
        {
            self.refresh_button_grayed_state(id);
            return;
        }
        let Some(component) = object.control().component() else {
            return;
        };
        let grayed = object.grayed();
        if component.controller("grayed").is_some() {
            self.set_controller_page_index(id, "grayed", i32::from(grayed));
            return;
        }
        let children = object.children().to_vec();
        for child in children {
            self.set_grayed(child, grayed);
        }
    }

    /// setup_beforeAdd assigns the serialized true value after a referenced
    /// component has been built, before instance gears/selection are configured.
    /// Native UIPackage._constructing suppresses controller gear tween creation.
    pub(crate) fn initialize_grayed(&mut self, id: GObjectId) {
        let previous = self.initializing_controllers;
        self.initializing_controllers = true;
        self.touch(id, dirty::COLOR);
        self.handle_grayed_changed(id);
        self.initializing_controllers = previous;
    }
}

#[cfg(test)]
#[path = "grayed_tests.rs"]
mod tests;
