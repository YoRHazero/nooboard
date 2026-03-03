use std::time::Duration;

use gpui::Animation;
use gpui_component::animation::cubic_bezier;

pub(crate) fn enter_animation() -> Animation {
    Animation::new(Duration::from_secs_f64(0.32)).with_easing(cubic_bezier(0.32, 0.72, 0.0, 1.0))
}

pub(crate) fn panel_toggle_animation() -> Animation {
    Animation::new(Duration::from_secs_f64(0.24)).with_easing(cubic_bezier(0.32, 0.72, 0.0, 1.0))
}
