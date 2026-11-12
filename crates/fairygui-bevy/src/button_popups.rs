use super::*;
use fairygui_core::geom::{Point, Rect as GuiRect, Size};

impl GuiRuntime {
    /// Popups, menus and combo dropdowns, from bottom to top. Windows are separate.
    pub fn popup_stack(&self) -> &[GObjectId] {
        &self.popups
    }

    /// Show an existing object at a root-local position. The root must exist;
    /// `ensure_root` supplies its viewport. Hiding retains the object and view.
    pub fn show_popup(&mut self, popup: GObjectId, at: Point) -> bool {
        let Some(root_id) = self.root_container() else {
            return false;
        };
        if !self.store.contains(popup) || self.is_descendant_of_or_self(root_id, popup) {
            return false;
        }
        if self.popups.contains(&popup) {
            self.hide_popup(Some(popup));
        }
        self.store.detach_from_stage(popup);
        if let Some(record) = self.roots.iter_mut().find(|root| root.object == popup) {
            record.attached = false;
        }
        self.root
            .as_mut()
            .unwrap()
            .show_popup(&mut self.store, popup, false);
        self.store.set_xy(popup, at.x, at.y);
        self.popups.push(popup);
        self.queue_subtree(popup);
        true
    }

    /// Hide this popup and any popups above it, or all popups when `None`.
    pub fn hide_popup(&mut self, popup: Option<GObjectId>) -> bool {
        let first = match popup {
            Some(popup) => match self.popups.iter().position(|id| *id == popup) {
                Some(index) => index,
                None => return false,
            },
            None => 0,
        };
        let removed = self.popups.split_off(first);
        let changed = !removed.is_empty();
        for popup in removed.into_iter().rev() {
            self.close_popup_entry(popup);
        }
        changed
    }

    /// Native GRoot checks once at pointer-down before a button toggles its link.
    /// Popups closed by that press cannot immediately reopen on the same press.
    pub(crate) fn check_popups(&mut self, target: Option<GObjectId>) {
        self.just_closed_popups.clear();
        self.popups.retain(|popup| self.store.contains(*popup));
        let keep = target
            .and_then(|target| {
                self.popups
                    .iter()
                    .rposition(|popup| self.is_descendant_of_or_self(target, *popup))
            })
            .map_or(0, |index| index + 1);
        let removed = self.popups.split_off(keep);
        for popup in removed.into_iter().rev() {
            self.close_popup_entry(popup);
            self.just_closed_popups.push(popup);
        }
    }

    fn close_popup_entry(&mut self, popup: GObjectId) {
        if let Some(root) = self.root.as_mut() {
            root.detach(&mut self.store, popup);
        }
        let combo = self.store.open_combo_ids().into_iter().find(|combo| {
            self.store
                .combo_popup(*combo)
                .is_some_and(|open| open.dropdown == popup)
        });
        if let Some(combo) = combo {
            self.store.close_combo_popup(combo);
        }
        if self.popup_menu.content() == Some(popup) {
            self.popup_menu.set_closed();
        }
        if self
            .pending_menu_click
            .as_ref()
            .is_some_and(|click| click.menu == popup)
        {
            self.pending_menu_click = None;
        }
    }

    pub(crate) fn toggle_button_popup(
        &mut self,
        button: GObjectId,
        anchor: GuiRect,
        viewport: Size,
    ) -> bool {
        let Some(ControlData::Button(data)) = self.store.object(button).map(|o| o.control()) else {
            return false;
        };
        let Some(popup) = data
            .linked_popup
            .filter(|popup| self.store.contains(*popup))
        else {
            return false;
        };
        if let Some(index) = self
            .windows
            .iter()
            .position(|window| window.owner() == popup)
        {
            return if self.windows[index].is_showing() {
                self.hide_window(index)
            } else {
                self.show_existing_window(index)
            };
        }
        if self.just_closed_popups.contains(&popup) {
            return true;
        }
        let root = self.ensure_root(viewport);
        let size = self.store.object(root).unwrap().size();
        let popup_size = self.store.object(popup).unwrap().size();
        let mut x = anchor.x;
        if x + popup_size.width > size.width {
            x += anchor.width - popup_size.width;
        }
        let mut y = anchor.y + anchor.height;
        // GRoot's Auto branch tests the anchor origin + popup height.
        if anchor.y + popup_size.height > size.height {
            y = anchor.y - popup_size.height - 1.0;
            if y < 0.0 {
                y = 0.0;
                x += anchor.width * 0.5;
            }
        }
        self.show_popup(popup, Point::new(x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_root_adopted_as_popup_spawns_under_root_and_reuses_its_view_on_reentry() {
        let package = FuiPackage::parse(
            "Basics",
            include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
        )
        .unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<FuiAsset>()
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<GuiFont>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
        let mut gui = GuiRuntime::default();
        let index = gui.create_root(handle, &package, "Demo_Text").unwrap();
        let popup = gui.root_object(index).unwrap();
        let root = gui.ensure_root(Size::new(800.0, 600.0));
        assert!(gui.show_popup(popup, Point::new(40.0, 50.0)));
        app.insert_non_send(gui).add_systems(
            Update,
            (
                spawn_pending_roots,
                spawn_pending_objects,
                sync_stage_hierarchy,
            )
                .chain(),
        );
        app.update();
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let entity = gui.entity_of(popup).unwrap();
        assert_eq!(
            app.world().get::<ChildOf>(entity).unwrap().parent(),
            gui.entity_of(root).unwrap()
        );
        assert_eq!(gui.store().object(popup).unwrap().parent(), Some(root));
        assert!(gui.store().is_on_stage(popup));
        assert!(
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .hide_popup(Some(popup))
        );
        app.update();
        assert!(app.world().get_entity(entity).is_ok());
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        assert!(
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .show_popup(popup, Point::ZERO)
        );
        app.update();
        assert_eq!(
            app.world().non_send::<GuiRuntime>().entity_of(popup),
            Some(entity)
        );
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn popup_position_matches_native_auto_flip_and_horizontal_adjustment() {
        let mut gui = GuiRuntime::default();
        let root = gui.ensure_root(Size::new(200.0, 150.0));
        let button = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Button);
        let popup = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Component);
        gui.store_mut().set_size(popup, 120.0, 80.0);
        gui.store_mut().set_button_linked_popup(button, Some(popup));
        assert!(gui.toggle_button_popup(
            button,
            GuiRect::new(160.0, 120.0, 30.0, 20.0),
            Size::ZERO
        ));
        assert_eq!(
            gui.store().object(popup).unwrap().position(),
            Point::new(70.0, 39.0)
        );
        gui.store_mut().set_size(popup, 120.0, 200.0);
        gui.toggle_button_popup(button, GuiRect::new(160.0, 40.0, 30.0, 20.0), Size::ZERO);
        assert_eq!(
            gui.store().object(popup).unwrap().position(),
            Point::new(85.0, 0.0)
        );
        assert_eq!(gui.store().object(popup).unwrap().parent(), Some(root));
        assert_eq!(gui.popup_stack(), &[popup]);
    }
}
