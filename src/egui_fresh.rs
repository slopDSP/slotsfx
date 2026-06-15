use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::Arc;

use crate::SlotsFxParams;

fn draw_knob(ui: &mut egui::Ui, setter: &ParamSetter, param: &FloatParam) {
    ui.vertical(|ui| {
        let size = egui::vec2(48.0, 48.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());

        let mut norm = param.modulated_normalized_value();
        if response.dragged() {
            norm -= response.drag_delta().y * 0.005;
            norm = norm.clamp(0.0, 1.0);
            setter.set_parameter_normalized(param, norm);
        }

        let painter = ui.painter();
        let center = rect.center();
        let radius = rect.width() / 2.0 - 3.0;

        painter.circle_filled(center, radius, egui::Color32::from_rgb(29, 29, 38));
        painter.circle_stroke(center, radius, egui::Stroke::new(2.0, egui::Color32::from_rgb(18, 18, 23)));

        let min_angle = (-135.0f32).to_radians();
        let max_angle = 135.0f32.to_radians();
        let angle = min_angle + norm * (max_angle - min_angle);
        let pointer = center + egui::vec2(angle.sin(), -angle.cos()) * (radius - 3.0);

        painter.line_segment([center, pointer], egui::Stroke::new(3.0, egui::Color32::from_rgb(0, 180, 216)));
        painter.circle_filled(center, 3.0, egui::Color32::WHITE);

        ui.add_space(4.0);
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("Gain").size(10.0).color(egui::Color32::from_rgb(140, 139, 159)));
        });
    });
}

pub fn create_editor(
    state: Arc<EguiState>,
    params: Arc<SlotsFxParams>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        state,
        params,
        |cx, _params| {
            let mut visuals = egui::Visuals::dark();
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(14, 13, 21);
            cx.set_visuals(visuals);
        },
        |ctx, setter, params| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("Slots").strong().color(egui::Color32::WHITE).size(24.0));
                    ui.heading(egui::RichText::new("FX").strong().color(egui::Color32::from_rgb(162, 125, 223)).size(24.0));
                });

                ui.add_space(30.0);

                ui.horizontal(|ui| {
                    ui.add_space(40.0);
                    draw_knob(ui, setter, &params.gain);
                });
            });
        },
    )
}
