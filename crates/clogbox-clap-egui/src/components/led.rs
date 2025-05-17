use egui::{emath, epaint, Color32, Response, Ui};
use std::ops::Add;

pub const DEFAULT_RADIUS: f32 = 10.0;

/// LED light component.
pub struct Led {
    /// Radius of the LED
    pub radius: f32,
    /// Current passing through the current. This is used to compute the color and brightness of the LED.
    /// Values ~1 produce [`Self::color`]. Values ~4 produce white.
    pub current: f32,
    /// Color of the LED.
    pub color: Color32,
    /// Background color (color of the casing around the LED)
    pub bg_color: Color32,
}

impl Default for Led {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            current: 0.0,
            color: Color32::RED,
            bg_color: Color32::BLACK,
        }
    }
}

impl egui::Widget for Led {
    fn ui(self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(emath::vec2(self.radius, self.radius), egui::Sense::empty());

        let center = response.rect.center();
        let radius = response.rect.size().min_elem() / 2.0;
        let color = led_color(self.bg_color, self.color, self.current);
        painter.circle_filled(center, radius, self.bg_color);
        painter.add(
            epaint::RectShape::new(
                response.rect.shrink(radius / 2.0),
                radius,
                color,
                epaint::Stroke::default(),
                epaint::StrokeKind::Middle,
            )
            .with_blur_width(radius),
        );

        response
    }
}

fn led_color(bg: Color32, fg: Color32, current: f32) -> Color32 {
    let brightness = 1.0 - (-current * 3.0).exp();
    let colored_component = fg.to_array().map(|b| b as f32 / 255.0).map(|f| f * brightness);
    let white_component = 1.0 - (-current / 9.0).exp();
    let [r, g, b, _] = colored_component
        .map(|f| f + white_component)
        .map(|f| f.clamp(0.0, 1.0) * 255.0)
        .map(|f| f.round() as u8);
    let add = Color32::from_rgb(r, g, b).additive();
    bg.add(add)
}
