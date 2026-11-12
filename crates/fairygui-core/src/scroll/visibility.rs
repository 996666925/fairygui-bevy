//! Scalar equivalent of ScrollPane.updateScrollBarVisible2 and its alpha tween.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BarVisibility {
    pub available: bool,
    pub visible: bool,
    pub alpha: f32,
    pub grip_dragging: bool,
    elapsed: Option<f64>,
}

impl Default for BarVisibility {
    fn default() -> Self {
        Self {
            available: true,
            visible: true,
            alpha: 1.,
            grip_dragging: false,
            elapsed: None,
        }
    }
}

impl BarVisibility {
    pub fn reset(&mut self, auto: bool, moving: bool) {
        self.alpha = 1.;
        self.elapsed = None;
        self.visible = self.available && (!auto || moving || self.grip_dragging);
    }

    pub fn refresh(&mut self, auto: bool, moving: bool) {
        if !self.available {
            self.visible = false;
            return;
        }
        if auto {
            self.elapsed = None;
        }
        if auto && !moving && !self.grip_dragging {
            if self.visible {
                self.elapsed = Some(0.);
            }
        } else {
            self.alpha = 1.;
            self.visible = true;
        }
    }

    pub fn advance(&mut self, dt: f32) -> bool {
        let before = (self.visible, self.alpha);
        if let Some(elapsed) = &mut self.elapsed {
            *elapsed += f64::from(dt);
            if *elapsed >= 1. {
                // Native completion resets alpha before hiding the display object.
                self.alpha = 1.;
                self.visible = false;
                self.elapsed = None;
            } else if *elapsed >= 0.5 {
                let remaining = 1. - (*elapsed - 0.5) / 0.5;
                self.alpha = (remaining * remaining) as f32;
            }
        }
        before != (self.visible, self.alpha)
    }
}
