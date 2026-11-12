use super::ScrollPane;

impl ScrollPane {
    /// Native currentPage uses the logical position; an exact half stays on
    /// the preceding page. It is independent of the current tween display.
    pub fn current_page_x(&self) -> i32 {
        self.current_page(self.pos.x, self.view_size.width)
    }

    pub fn current_page_y(&self) -> i32 {
        self.current_page(self.pos.y, self.view_size.height)
    }

    fn current_page(&self, position: f32, size: f32) -> i32 {
        if !self.page_mode || !size.is_finite() || size <= 0. {
            return 0;
        }
        let page = (position / size).floor();
        (page
            + if position - page * size > size * 0.5 {
                1.
            } else {
                0.
            }) as i32
    }

    pub fn set_current_page_x(&mut self, page: i32, animated: bool) -> bool {
        if !self.page_mode || self.max_scroll_pos().x <= 0. {
            return false;
        }
        self.set_pos_animated(page as f32 * self.view_size.width, self.pos.y, animated)
    }

    pub fn set_current_page_y(&mut self, page: i32, animated: bool) -> bool {
        if !self.page_mode || self.max_scroll_pos().y <= 0. {
            return false;
        }
        self.set_pos_animated(self.pos.x, page as f32 * self.view_size.height, animated)
    }
}
