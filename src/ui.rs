use std::sync::Arc;
use std::sync::Mutex;
use rtrb::Producer;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use nih_plug::prelude::*;

pub struct SlotsEditorData {
    pub params: Arc<crate::SlotsFxParams>,
    pub nam_model_name: Arc<Mutex<String>>,
    pub nam_model_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    pub cab_ir_name: Arc<Mutex<String>>,
    pub cab_ir_path: Arc<Mutex<Option<std::path::PathBuf>>>,

    pub nam_sender: Arc<Mutex<Option<Producer<(nam_rs::Model, nam_rs::Model, Option<f32>)>>>>,
    pub cab_sender: Arc<Mutex<Option<Producer<(Vec<f32>, Vec<f32>)>>>>,
    pub routing_sender: Arc<Mutex<Option<Producer<[usize; 5]>>>>,
    pub blocks_sender: Arc<Mutex<Option<Producer<Vec<crate::dsp::SlotConfig>>>>>,
    /// Sender for cab normalize toggle: (normalize, ir_l, ir_r)
    pub cab_normalize_sender: Arc<Mutex<Option<Producer<(bool, Option<Vec<f32>>, Option<Vec<f32>>)>>>>,
    pub routing_order: Arc<Mutex<[usize; 5]>>,

    pub dsp_metrics: Arc<crate::DspMetrics>,
    pub instance_id: usize,
}

impl Clone for SlotsEditorData {
    fn clone(&self) -> Self {
        Self {
            params: self.params.clone(),
            nam_model_name: self.nam_model_name.clone(),
            nam_model_path: self.nam_model_path.clone(),
            cab_ir_name: self.cab_ir_name.clone(),
            cab_ir_path: self.cab_ir_path.clone(),
            nam_sender: self.nam_sender.clone(),
            cab_sender: self.cab_sender.clone(),
            routing_sender: self.routing_sender.clone(),
            blocks_sender: self.blocks_sender.clone(),
            cab_normalize_sender: self.cab_normalize_sender.clone(),
            routing_order: self.routing_order.clone(),
            dsp_metrics: self.dsp_metrics.clone(),
            instance_id: self.instance_id,
        }
    }
}

fn draw_knob(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    color: egui::Color32,
) {
    ui.vertical(|ui| {
        let desired_size = egui::vec2(44.0, 44.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::drag());

        let mut norm_val = param.modulated_normalized_value();
        if response.dragged() {
            let delta = response.drag_delta().y;
            norm_val -= delta * 0.005;
            norm_val = norm_val.clamp(0.0, 1.0);
            setter.set_parameter_normalized(param, norm_val);
        }

        let painter = ui.painter();
        let center = rect.center();
        let radius = rect.width() / 2.0 - 2.0;

        painter.circle_filled(center, radius, egui::Color32::from_rgb(29, 29, 38));
        painter.circle_stroke(center, radius, egui::Stroke::new(2.5, egui::Color32::from_rgb(18, 18, 23)));

        let min_angle = -135.0f32.to_radians();
        let max_angle = 135.0f32.to_radians();
        let current_angle = min_angle + norm_val * (max_angle - min_angle);

        let pointer_len = radius - 3.0;
        let pointer_end = center + egui::vec2(current_angle.sin(), -current_angle.cos()) * pointer_len;
        painter.line_segment(
            [center, pointer_end],
            egui::Stroke::new(3.0, color),
        );

        painter.circle_filled(center, 4.0, egui::Color32::from_rgb(255, 255, 255));

        ui.add_space(4.0);
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(label).size(9.0).color(egui::Color32::from_rgb(140, 139, 159)).strong());
        });
    });
}

fn draw_bypass_switch(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &BoolParam,
    accent_color: egui::Color32,
) {
    let active = !param.value();

    ui.vertical(|ui| {
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(26.0, 12.0), egui::Sense::hover());
        let center = rect.center();
        let painter = ui.painter();
        if active {
            painter.circle_filled(center, 3.5, accent_color);
            painter.circle_filled(center, 7.0, accent_color.linear_multiply(0.25));
        } else {
            painter.circle_filled(center, 3.5, egui::Color32::from_rgb(40, 40, 50));
        }

        ui.add_space(4.0);

        let button_text = if active { "ON" } else { "BYP" };
        let btn_color = if active {
            egui::Color32::from_rgb(30, 40, 50)
        } else {
            egui::Color32::from_rgb(20, 20, 25)
        };

        let btn = egui::Button::new(
            egui::RichText::new(button_text)
                .size(8.0)
                .color(if active { egui::Color32::WHITE } else { egui::Color32::GRAY })
                .strong()
        )
        .fill(btn_color)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));

        if ui.add_sized(egui::vec2(26.0, 24.0), btn).clicked() {
            setter.set_parameter(param, !param.value());
        }
    });
}

fn draw_reorder_handle(
    ui: &mut egui::Ui,
    rack_pos: usize,
    new_order: &mut [usize; 5],
    order_changed: &mut bool,
) {
    ui.vertical(|ui| {
        let up_btn = egui::Button::new(egui::RichText::new("▲").size(7.0))
            .fill(egui::Color32::from_rgb(24, 23, 33))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));
        if ui.add_sized(egui::vec2(18.0, 14.0), up_btn).clicked() && rack_pos > 0 {
            new_order.swap(rack_pos, rack_pos - 1);
            *order_changed = true;
        }

        ui.add_space(4.0);

        let desired_size = egui::vec2(12.0, 16.0);
        let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let painter = ui.painter();
        let dot_color = egui::Color32::from_rgb(95, 105, 128);
        let start_x = rect.min.x + 2.0;
        let start_y = rect.min.y + 2.0;

        for row in 0..3 {
            let y = start_y + (row as f32) * 5.0;
            painter.circle_filled(egui::pos2(start_x, y), 1.5, dot_color);
            painter.circle_filled(egui::pos2(start_x + 6.0, y), 1.5, dot_color);
        }

        ui.add_space(4.0);

        let down_btn = egui::Button::new(egui::RichText::new("▼").size(7.0))
            .fill(egui::Color32::from_rgb(24, 23, 33))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));
        if ui.add_sized(egui::vec2(18.0, 14.0), down_btn).clicked() && rack_pos < 4 {
            new_order.swap(rack_pos, rack_pos + 1);
            *order_changed = true;
        }
    });
}

pub fn create_editor(
    egui_state: Arc<EguiState>,
    data: SlotsEditorData,
) -> Option<Box<dyn nih_plug::prelude::Editor>> {
    create_egui_editor(
        egui_state,
        data,
        |cx, _data| {
            let mut visuals = egui::Visuals::dark();
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(44, 27, 77);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 40, 100);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(21, 20, 29);
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(14, 13, 21);
            visuals.selection.bg_fill = egui::Color32::from_rgb(0, 180, 216);
            cx.set_visuals(visuals);
        },
        |ctx, setter, data| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_max_width(900.0);
                ui.set_max_height(780.0);

                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("Slots").strong().color(egui::Color32::WHITE).size(32.0));
                    ui.heading(egui::RichText::new("FX").strong().color(egui::Color32::from_rgb(162, 125, 223)).size(32.0));
                    ui.label(egui::RichText::new("LIVE PROCESSOR").size(10.0).color(egui::Color32::from_rgb(162, 125, 223)).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(20.0);
                        if ui.button(egui::RichText::new("File Loader (.nam)").strong().color(egui::Color32::WHITE)).clicked() {
                            let nam_sender = data.nam_sender.clone();
                            let nam_model_name = data.nam_model_name.clone();
                            let nam_model_path = data.nam_model_path.clone();
                            std::thread::spawn(move || {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("NAM Model", &["nam"])
                                    .pick_file()
                                {
                                    if let Ok(model_file) = nam_rs::NamModel::from_file(&path) {
                                        let loudness = model_file.loudness();
                                        let model_l = nam_rs::Model::from_nam(&model_file);
                                        let model_r = nam_rs::Model::from_nam(&model_file);

                                        if let (Ok(ml), Ok(mr)) = (model_l, model_r) {
                                            if let Some(ref mut sender) = *nam_sender.lock().unwrap() {
                                                let _ = sender.push((ml, mr, loudness));
                                            }
                                            *nam_model_name.lock().unwrap() = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                                            *nam_model_path.lock().unwrap() = Some(path);
                                        }
                                    }
                                }
                            });
                        }

                        let nam_name = data.nam_model_name.lock().unwrap().clone();
                        if !nam_name.is_empty() {
                            ui.colored_label(egui::Color32::from_rgb(224, 122, 95), format!("Model: {}", nam_name));
                        }
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(15.0);

                ui.horizontal(|ui| {
                    ui.add_space(15.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(550.0, 600.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let mut order_changed = false;
                            let mut new_order = *data.routing_order.lock().unwrap();

                            for rack_pos in 0..5 {
                                let block_idx = new_order[rack_pos];

                                match block_idx {
                                    0 => {
                                        let frame_color = egui::Color32::from_rgb(15, 32, 46);
                                        let accent = egui::Color32::from_rgb(0, 210, 255);

                                        egui::Frame::NONE
                                            .fill(frame_color)
                                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(31, 50, 70)))
                                            .inner_margin(12.0)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.set_min_width(550.0);
                                                ui.set_min_height(76.0);
                                                ui.horizontal(|ui| {
                                                    draw_reorder_handle(ui, rack_pos, &mut new_order, &mut order_changed);
                                                    ui.add_space(10.0);
                                                    draw_bypass_switch(ui, setter, &data.params.pitch_bypass, accent);
                                                    ui.add_space(15.0);

                                                    ui.vertical(|ui| {
                                                        ui.label(egui::RichText::new("Pitch Shifter").strong().size(16.0).color(egui::Color32::WHITE));
                                                    });

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.add_space(20.0);
                                                        draw_knob(ui, setter, &data.params.pitch_gain, "Gain", accent);
                                                    });
                                                });
                                            });
                                    }
                                    1 => {
                                        let frame_color = egui::Color32::from_rgb(34, 23, 53);
                                        let accent = egui::Color32::from_rgb(224, 122, 95);

                                        egui::Frame::NONE
                                            .fill(frame_color)
                                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(48, 26, 78)))
                                            .inner_margin(12.0)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.set_min_width(550.0);
                                                ui.set_min_height(76.0);
                                                ui.horizontal(|ui| {
                                                    draw_reorder_handle(ui, rack_pos, &mut new_order, &mut order_changed);
                                                    ui.add_space(10.0);
                                                    draw_bypass_switch(ui, setter, &data.params.nam_bypass, accent);
                                                    ui.add_space(15.0);

                                                    ui.vertical(|ui| {
                                                        ui.label(egui::RichText::new("Dual NAM").strong().size(16.0).color(egui::Color32::WHITE));
                                                        let nam_name = data.nam_model_name.lock().unwrap().clone();
                                                        if !nam_name.is_empty() {
                                                            ui.label(egui::RichText::new(format!("Profile: {}", nam_name)).size(10.0).color(accent));
                                                        } else {
                                                            ui.label(egui::RichText::new("No NAM Loaded (Bypassed)").size(10.0).color(egui::Color32::GRAY));
                                                        }
                                                    });

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.add_space(20.0);
                                                        draw_knob(ui, setter, &data.params.nam_gain, "Gain", accent);
                                                    });
                                                });
                                            });
                                    }
                                    2 => {
                                        let frame_color = egui::Color32::from_rgb(34, 28, 38);
                                        let accent = egui::Color32::from_rgb(224, 122, 95);

                                        egui::Frame::NONE
                                            .fill(frame_color)
                                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(45, 32, 51)))
                                            .inner_margin(12.0)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.set_min_width(550.0);
                                                ui.set_min_height(76.0);
                                                ui.horizontal(|ui| {
                                                    draw_reorder_handle(ui, rack_pos, &mut new_order, &mut order_changed);
                                                    ui.add_space(10.0);
                                                    draw_bypass_switch(ui, setter, &data.params.cab_bypass, accent);
                                                    ui.add_space(15.0);

                                                    ui.vertical(|ui| {
                                                        ui.label(egui::RichText::new("Cab IR").strong().size(16.0).color(egui::Color32::WHITE));
                                                        let cab_name = data.cab_ir_name.lock().unwrap().clone();
                                                        ui.label(egui::RichText::new(format!("IR: {}", cab_name)).size(10.0).color(accent));
                                                    });

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.add_space(20.0);
                                                        draw_knob(ui, setter, &data.params.cab_gain, "Gain", accent);
                                                    });
                                                });
                                            });
                                    }
                                    3 => {
                                        let frame_color = egui::Color32::from_rgb(8, 20, 31);
                                        let accent = egui::Color32::from_rgb(0, 210, 255);

                                        egui::Frame::NONE
                                            .fill(frame_color)
                                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(26, 51, 78)))
                                            .inner_margin(12.0)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.set_min_width(550.0);
                                                ui.set_min_height(76.0);
                                                ui.horizontal(|ui| {
                                                    draw_reorder_handle(ui, rack_pos, &mut new_order, &mut order_changed);
                                                    ui.add_space(10.0);
                                                    draw_bypass_switch(ui, setter, &data.params.delay_bypass, accent);
                                                    ui.add_space(15.0);

                                                    ui.vertical(|ui| {
                                                        ui.label(egui::RichText::new("Ping-Pong Delay").strong().size(16.0).color(egui::Color32::WHITE));
                                                    });

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.add_space(10.0);
                                                        draw_knob(ui, setter, &data.params.delay_feedback, "Feedback", accent);
                                                        ui.add_space(10.0);
                                                        draw_knob(ui, setter, &data.params.delay_mix, "Mix", accent);
                                                    });
                                                });
                                            });
                                    }
                                    4 => {
                                        let frame_color = egui::Color32::from_rgb(29, 26, 53);
                                        let accent = egui::Color32::from_rgb(162, 125, 223);

                                        egui::Frame::NONE
                                            .fill(frame_color)
                                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(44, 32, 75)))
                                            .inner_margin(12.0)
                                            .corner_radius(10)
                                            .show(ui, |ui| {
                                                ui.set_min_width(550.0);
                                                ui.set_min_height(76.0);
                                                ui.horizontal(|ui| {
                                                    draw_reorder_handle(ui, rack_pos, &mut new_order, &mut order_changed);
                                                    ui.add_space(10.0);
                                                    draw_bypass_switch(ui, setter, &data.params.reverb_bypass, accent);
                                                    ui.add_space(15.0);

                                                    ui.vertical(|ui| {
                                                        ui.label(egui::RichText::new("Cosmic Shimmer").strong().size(16.0).color(egui::Color32::WHITE));
                                                        ui.label(egui::RichText::new("Reverb").strong().size(12.0).color(egui::Color32::WHITE));
                                                    });

                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.add_space(10.0);
                                                        draw_knob(ui, setter, &data.params.reverb_space, "Reverb", accent);
                                                        ui.add_space(10.0);
                                                        draw_knob(ui, setter, &data.params.reverb_mix, "Mix", accent);
                                                    });
                                                });
                                            });
                                    }
                                    _ => {}
                                }
                                ui.add_space(10.0);
                            }

                            if order_changed {
                                *data.routing_order.lock().unwrap() = new_order;
                                if let Some(ref mut sender) = *data.routing_sender.lock().unwrap() {
                                    let _ = sender.push(new_order);
                                }
                            }
                        }
                    );

                    ui.add_space(15.0);

                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(20, 19, 28))
                        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(38, 37, 52)))
                        .inner_margin(16.0)
                        .corner_radius(10)
                        .show(ui, |ui| {
                            ui.set_min_width(235.0);
                            ui.set_min_height(536.0);

                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("Cabinet Profiler").strong().size(18.0).color(egui::Color32::WHITE));
                                ui.add_space(20.0);

                                let start_btn = egui::Button::new(
                                    egui::RichText::new("Start Profile")
                                        .strong()
                                        .size(15.0)
                                        .color(egui::Color32::from_rgb(0, 255, 136))
                                )
                                .fill(egui::Color32::from_rgb(17, 34, 26))
                                .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 255, 136)))
                                .corner_radius(21);

                                if ui.add_sized(egui::vec2(195.0, 42.0), start_btn).clicked() {
                                }

                                ui.add_space(20.0);
                                ui.separator();
                                ui.add_space(20.0);

                                ui.label(egui::RichText::new("Load Cabinet Impulse Response:").size(11.0).color(egui::Color32::GRAY));
                                ui.add_space(10.0);

                                if ui.button(egui::RichText::new("Browse WAV IR").strong().color(egui::Color32::WHITE)).clicked() {
                                    let cab_sender = data.cab_sender.clone();
                                    let cab_ir_name = data.cab_ir_name.clone();
                                    let cab_ir_path = data.cab_ir_path.clone();
                                    std::thread::spawn(move || {
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter("WAV Audio", &["wav"])
                                            .pick_file()
                                        {
                                            if let Ok(mut reader) = hound::WavReader::open(&path) {
                                                let spec = reader.spec();
                                                let samples: Vec<f32> = match spec.sample_format {
                                                    hound::SampleFormat::Float => {
                                                        reader.samples::<f32>().filter_map(Result::ok).collect()
                                                    }
                                                    hound::SampleFormat::Int => {
                                                        let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
                                                        reader.samples::<i32>()
                                                            .filter_map(Result::ok)
                                                            .map(|s| s as f32 / max_val)
                                                            .collect()
                                                    }
                                                };

                                                if !samples.is_empty() {
                                                    let ir_left: Vec<f32>;
                                                    let ir_right: Vec<f32>;

                                                    if spec.channels == 2 {
                                                        ir_left = samples.iter().step_by(2).copied().collect();
                                                        ir_right = samples.iter().skip(1).step_by(2).copied().collect();
                                                    } else {
                                                        ir_left = samples.clone();
                                                        ir_right = samples;
                                                    }

                                                    if let Some(ref mut sender) = *cab_sender.lock().unwrap() {
                                                        let _ = sender.push((ir_left, ir_right));
                                                    }
                                                    *cab_ir_name.lock().unwrap() = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                                                    *cab_ir_path.lock().unwrap() = Some(path);
                                                }
                                            }
                                        }
                                    });
                                }

                                ui.add_space(10.0);
                                let cab_name = data.cab_ir_name.lock().unwrap().clone();
                                ui.colored_label(egui::Color32::from_rgb(224, 122, 95), format!("Loaded IR: {}", cab_name));
                            });
                        });
                });
            });
        },
    )
}
