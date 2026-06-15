use std::sync::Arc;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use nih_plug::prelude::*;

use crate::ui::SlotsEditorData;

struct SlotMeta {
    title: &'static str,
    knobs: &'static [&'static str],
    color: (u8, u8, u8),
    accent: (u8, u8, u8),
}

fn slot_meta(slot_type: &str) -> SlotMeta {
    match slot_type {
        "amp" => SlotMeta {
            title: "Amp",
            knobs: &["amp_gain", "amp_bass", "amp_middle", "amp_high", "amp_output"],
            color: (34, 23, 53),
            accent: (224, 122, 95),
        },
        "cab" => SlotMeta {
            title: "Cab",
            knobs: &["cab_gain", "cab_position", "cab_size"],
            color: (34, 28, 38),
            accent: (224, 122, 95),
        },
        "pitch" => SlotMeta {
            title: "Pitch Shifter",
            knobs: &["pitch_gain", "pitch_semi", "pitch_mix"],
            color: (15, 32, 46),
            accent: (0, 210, 255),
        },
        "delay" => SlotMeta {
            title: "Delay",
            knobs: &["delay_time", "delay_feedback", "delay_ducking", "delay_mix"],
            color: (8, 20, 31),
            accent: (0, 210, 255),
        },
        "verb" | "shimmer" => SlotMeta {
            title: "Reverb",
            knobs: &["reverb_space", "reverb_ducking", "reverb_mix"],
            color: (29, 26, 53),
            accent: (162, 125, 223),
        },
        "gate" => SlotMeta {
            title: "Gate",
            knobs: &["gate_threshold", "gate_attack", "gate_release"],
            color: (17, 34, 26),
            accent: (74, 222, 128),
        },
        "error" => SlotMeta {
            title: "Bitcrusher",
            knobs: &["bitcrush_bits", "bitcrush_downsample", "bitcrush_mix"],
            color: (30, 15, 15),
            accent: (255, 69, 69),
        },
        "od" => SlotMeta {
            title: "Overdrive",
            knobs: &["overdrive_drive", "overdrive_tone", "overdrive_level"],
            color: (30, 24, 10),
            accent: (255, 170, 0),
        },
        "eq" => SlotMeta {
            title: "EQ",
            knobs: &["eq_low_gain", "eq_mid_gain", "eq_high_gain"],
            color: (10, 30, 24),
            accent: (0, 255, 170),
        },
        _ => SlotMeta {
            title: "Slot",
            knobs: &[],
            color: (20, 20, 30),
            accent: (100, 100, 140),
        },
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
        let desired_size = egui::vec2(36.0, 36.0);
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
        painter.circle_stroke(center, radius, egui::Stroke::new(2.0, egui::Color32::from_rgb(18, 18, 23)));

        let min_angle = (-135.0f32).to_radians();
        let max_angle = 135.0f32.to_radians();
        let current_angle = min_angle + norm_val * (max_angle - min_angle);
        let pointer_len = radius - 3.0;
        let pointer_end = center + egui::vec2(current_angle.sin(), -current_angle.cos()) * pointer_len;
        painter.line_segment([center, pointer_end], egui::Stroke::new(2.5, color));
        painter.circle_filled(center, 3.0, egui::Color32::WHITE);

        ui.add_space(2.0);
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(label).size(8.0).color(egui::Color32::from_rgb(140, 139, 159)));
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
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 10.0), egui::Sense::hover());
        let center = rect.center();
        let painter = ui.painter();
        if active {
            painter.circle_filled(center, 3.0, accent_color);
            painter.circle_filled(center, 6.0, accent_color.linear_multiply(0.25));
        } else {
            painter.circle_filled(center, 3.0, egui::Color32::from_rgb(40, 40, 50));
        }
        ui.add_space(2.0);
        let btn_text = if active { "ON" } else { "BYP" };
        let btn = egui::Button::new(
            egui::RichText::new(btn_text).size(7.0)
                .color(if active { egui::Color32::WHITE } else { egui::Color32::GRAY })
        )
        .fill(if active { egui::Color32::from_rgb(30, 40, 50) } else { egui::Color32::from_rgb(20, 20, 25) })
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));
        if ui.add_sized(egui::vec2(22.0, 20.0), btn).clicked() {
            setter.set_parameter(param, !param.value());
        }
    });
}

pub fn create_editor(
    egui_state: Arc<EguiState>,
    data: SlotsEditorData,
) -> Option<Box<dyn Editor>> {
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
                ui.set_max_width(960.0);
                ui.set_max_height(800.0);

                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.heading(egui::RichText::new("Slots").strong().color(egui::Color32::WHITE).size(32.0));
                    ui.heading(egui::RichText::new("FX").strong().color(egui::Color32::from_rgb(162, 125, 223)).size(32.0));
                    ui.label(egui::RichText::new("LIVE PROCESSOR").size(10.0).color(egui::Color32::from_rgb(162, 125, 223)).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        draw_knob(ui, setter, &data.params.mix, "Mix", egui::Color32::from_rgb(162, 125, 223));
                        ui.add_space(8.0);
                        draw_knob(ui, setter, &data.params.output_gain, "Output", egui::Color32::from_rgb(224, 122, 95));
                        ui.add_space(8.0);
                        draw_knob(ui, setter, &data.params.input_gain, "Input", egui::Color32::from_rgb(224, 122, 95));
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.add_space(15.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(580.0, 600.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let new_order = *data.routing_order.lock().unwrap();
                            let slots_json = data.params.slots_json.lock().unwrap().clone();
                            let slots: Vec<serde_json::Value> = serde_json::from_str(&slots_json).unwrap_or_default();

                            for (rank, &block_idx) in new_order.iter().enumerate() {
                                if block_idx >= slots.len() { continue; }
                                let slot = &slots[block_idx];
                                let _slot_id = slot.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let slot_type = slot.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                let name = slot.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let meta = slot_meta(slot_type);

                                let frame_color = egui::Color32::from_rgb(meta.color.0, meta.color.1, meta.color.2);
                                let accent = egui::Color32::from_rgb(meta.accent.0, meta.accent.1, meta.accent.2);

                                egui::Frame::NONE
                                    .fill(frame_color)
                                    .stroke(egui::Stroke::new(1.5, accent.linear_multiply(0.5)))
                                    .inner_margin(10.0)
                                    .corner_radius(8)
                                    .show(ui, |ui| {
                                        ui.set_min_width(560.0);
                                        ui.set_min_height(60.0);
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                let up = egui::Button::new(egui::RichText::new("▲").size(6.0))
                                                    .fill(egui::Color32::from_rgb(24, 23, 33))
                                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));
                                                if ui.add_sized(egui::vec2(16.0, 12.0), up).clicked() && rank > 0 {
                                                }
                                                ui.add_space(2.0);
                                                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 12.0), egui::Sense::hover());
                                                let p = ui.painter();
                                                let dot_c = egui::Color32::from_rgb(95, 105, 128);
                                                for row in 0..3 {
                                                    p.circle_filled(egui::pos2(r.min.x + 1.0, r.min.y + (row as f32) * 4.0), 1.2, dot_c);
                                                    p.circle_filled(egui::pos2(r.min.x + 5.0, r.min.y + (row as f32) * 4.0), 1.2, dot_c);
                                                }
                                                ui.add_space(2.0);
                                                let dn = egui::Button::new(egui::RichText::new("▼").size(6.0))
                                                    .fill(egui::Color32::from_rgb(24, 23, 33))
                                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 44, 61)));
                                                ui.add_sized(egui::vec2(16.0, 12.0), dn);
                                            });
                                            ui.add_space(8.0);

                                            let bypass_param: Option<&BoolParam> = match slot_type {
                                                "pitch" => Some(&data.params.pitch_bypass),
                                                "amp" => Some(&data.params.nam_bypass),
                                                "cab" => Some(&data.params.cab_bypass),
                                                "delay" => Some(&data.params.delay_bypass),
                                                "verb" | "shimmer" => Some(&data.params.reverb_bypass),
                                                _ => None,
                                            };
                                            if let Some(bp) = bypass_param {
                                                draw_bypass_switch(ui, setter, bp, accent);
                                            }
                                            ui.add_space(10.0);

                                            ui.vertical(|ui| {
                                                ui.label(egui::RichText::new(meta.title).strong().size(14.0).color(egui::Color32::WHITE));
                                                if !name.is_empty() {
                                                    ui.label(egui::RichText::new(name).size(9.0).color(accent));
                                                }
                                            });

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                for knob_id in meta.knobs {
                                                    let param: Option<&FloatParam> = match *knob_id {
                                                        "amp_gain" => Some(&data.params.nam_gain),
                                                        "amp_bass" => Some(&data.params.amp_bass),
                                                        "amp_middle" => Some(&data.params.amp_middle),
                                                        "amp_high" => Some(&data.params.amp_high),
                                                        "amp_output" => Some(&data.params.amp_output),
                                                        "cab_gain" => Some(&data.params.cab_gain),
                                                        "cab_position" => Some(&data.params.cab_position),
                                                        "cab_size" => Some(&data.params.cab_size),
                                                        "pitch_gain" => Some(&data.params.pitch_gain),
                                                        "pitch_semi" => Some(&data.params.pitch_semi),
                                                        "pitch_mix" => Some(&data.params.pitch_mix),
                                                        "delay_time" => Some(&data.params.delay_time),
                                                        "delay_feedback" => Some(&data.params.delay_feedback),
                                                        "delay_mix" => Some(&data.params.delay_mix),
                                                        "delay_ducking" => Some(&data.params.delay_ducking),
                                                        "reverb_space" => Some(&data.params.reverb_space),
                                                        "reverb_mix" => Some(&data.params.reverb_mix),
                                                        "reverb_shimmer" => Some(&data.params.reverb_shimmer),
                                                        "reverb_ducking" => Some(&data.params.reverb_ducking),
                                                        "gate_threshold" => Some(&data.params.gate_threshold),
                                                        "gate_attack" => Some(&data.params.gate_attack),
                                                        "gate_release" => Some(&data.params.gate_release),
                                                        "bitcrush_bits" => Some(&data.params.bitcrush_bits),
                                                        "bitcrush_downsample" => Some(&data.params.bitcrush_downsample),
                                                        "bitcrush_mix" => Some(&data.params.bitcrush_mix),
                                                        "overdrive_drive" => Some(&data.params.overdrive_drive),
                                                        "overdrive_tone" => Some(&data.params.overdrive_tone),
                                                        "overdrive_level" => Some(&data.params.overdrive_level),
                                                        "eq_low_gain" => Some(&data.params.eq_low_gain),
                                                        "eq_mid_gain" => Some(&data.params.eq_mid_gain),
                                                        "eq_high_gain" => Some(&data.params.eq_high_gain),
                                                        _ => None,
                                                    };
                                                    if let Some(p) = param {
                                                        let spec_label = match *knob_id {
                                                            "amp_gain" | "cab_gain" | "pitch_gain" => "Gain",
                                                            "amp_bass" => "Bass",
                                                            "amp_middle" => "Mid",
                                                            "amp_high" => "High",
                                                            "amp_output" => "Out",
                                                            "cab_position" => "Pos",
                                                            "cab_size" => "Size",
                                                            "pitch_semi" => "Semi",
                                                            "pitch_mix" => "Mix",
                                                            "delay_time" => "Time",
                                                            "delay_feedback" => "Fdbk",
                                                            "delay_ducking" => "Duck",
                                                            "delay_mix" => "Mix",
                                                            "reverb_space" => "Space",
                                                            "reverb_shimmer" => "Shim",
                                                            "reverb_ducking" => "Duck",
                                                            "reverb_mix" => "Mix",
                                                            "gate_threshold" => "Thresh",
                                                            "gate_attack" => "Atk",
                                                            "gate_release" => "Rel",
                                                            "bitcrush_bits" => "Bits",
                                                            "bitcrush_downsample" => "Down",
                                                            "bitcrush_mix" => "Mix",
                                                            "overdrive_drive" => "Drive",
                                                            "overdrive_tone" => "Tone",
                                                            "overdrive_level" => "Level",
                                                            "eq_low_gain" => "LoG",
                                                            "eq_mid_gain" => "MidG",
                                                            "eq_high_gain" => "HiG",
                                                            _ => "???",
                                                        };
                                                        ui.add_space(6.0);
                                                        draw_knob(ui, setter, p, spec_label, accent);
                                                    }
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                        }
                    );

                    ui.add_space(10.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(240.0, 600.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(20, 19, 28))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 37, 52)))
                                .inner_margin(12.0)
                                .corner_radius(8)
                                .show(ui, |ui| {
                                    ui.set_min_width(220.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("Cabinet Profiler").strong().size(14.0).color(egui::Color32::WHITE));
                                        ui.add_space(8.0);

                                        let start_btn = egui::Button::new(
                                            egui::RichText::new("Start Profile").strong().size(13.0)
                                                .color(egui::Color32::from_rgb(0, 255, 136))
                                        )
                                        .fill(egui::Color32::from_rgb(17, 34, 26))
                                        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 255, 136)))
                                        .corner_radius(16);
                                        if ui.add_sized(egui::vec2(180.0, 36.0), start_btn).clicked() {
                                            if let Ok(reg) = crate::registry::get_registry().lock() {
                                                if let Some(inst) = reg.instances.get(&data.instance_id) {
                                                    inst.sweep_trigger.store(true, std::sync::atomic::Ordering::Relaxed);
                                                }
                                            }
                                        }

                                        ui.add_space(16.0);
                                        ui.separator();
                                        ui.add_space(12.0);

                                        ui.label(egui::RichText::new("Load IR:").size(10.0).color(egui::Color32::GRAY));
                                        ui.add_space(6.0);
                                        if ui.button(egui::RichText::new("Browse WAV IR").strong().color(egui::Color32::WHITE))
                                            .clicked()
                                        {
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
                                                            let (ir_l, ir_r) = if spec.channels == 2 {
                                                                (
                                                                    samples.iter().step_by(2).copied().collect(),
                                                                    samples.iter().skip(1).step_by(2).copied().collect(),
                                                                )
                                                            } else {
                                                                (samples.clone(), samples)
                                                            };
                                                            if let Some(ref mut sender) = *cab_sender.lock().unwrap() {
                                                                let _ = sender.push((ir_l, ir_r));
                                                            }
                                                            *cab_ir_name.lock().unwrap() = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                                                            *cab_ir_path.lock().unwrap() = Some(path);
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        ui.add_space(6.0);
                                        let cab_name = data.cab_ir_name.lock().unwrap().clone();
                                        if !cab_name.is_empty() {
                                            ui.label(egui::RichText::new(format!("IR: {}", cab_name)).size(9.0).color(egui::Color32::from_rgb(224, 122, 95)));
                                        }
                                    });
                                });

                            ui.add_space(10.0);

                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(20, 19, 28))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 37, 52)))
                                .inner_margin(12.0)
                                .corner_radius(8)
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("Load NAM Model").strong().size(12.0).color(egui::Color32::WHITE));
                                        ui.add_space(6.0);
                                        if ui.button(egui::RichText::new("Browse (.nam)").strong().color(egui::Color32::WHITE))
                                            .clicked()
                                        {
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
                                                        let ml = nam_rs::Model::from_nam(&model_file);
                                                        let mr = nam_rs::Model::from_nam(&model_file);
                                                        if let (Ok(ml), Ok(mr)) = (ml, mr) {
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
                                            ui.add_space(4.0);
                                            ui.label(egui::RichText::new(format!("Model: {}", nam_name)).size(9.0).color(egui::Color32::from_rgb(224, 122, 95)));
                                        }
                                    });
                                });

                            ui.add_space(10.0);

                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(20, 19, 28))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 37, 52)))
                                .inner_margin(12.0)
                                .corner_radius(8)
                                .show(ui, |ui| {
                                    ui.set_min_width(220.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("Macros").strong().size(12.0).color(egui::Color32::WHITE));
                                        ui.add_space(8.0);
                                        ui.horizontal(|ui| {
                                            draw_knob(ui, setter, &data.params.macro_1, "M1", egui::Color32::from_rgb(0, 180, 216));
                                            ui.add_space(4.0);
                                            draw_knob(ui, setter, &data.params.macro_2, "M2", egui::Color32::from_rgb(0, 180, 216));
                                        });
                                        ui.horizontal(|ui| {
                                            draw_knob(ui, setter, &data.params.macro_3, "M3", egui::Color32::from_rgb(0, 180, 216));
                                            ui.add_space(4.0);
                                            draw_knob(ui, setter, &data.params.macro_4, "M4", egui::Color32::from_rgb(0, 180, 216));
                                        });
                                    });
                                });

                            ui.add_space(10.0);

                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(20, 19, 28))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 37, 52)))
                                .inner_margin(12.0)
                                .corner_radius(8)
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label(egui::RichText::new("Snapshots").strong().size(12.0).color(egui::Color32::WHITE));
                                        ui.add_space(6.0);
                                        let snap_val = data.params.snapshot.value();
                                        ui.horizontal(|ui| {
                                            for i in 0..8 {
                                                let selected = (snap_val as usize) == i;
                                                let btn = egui::Button::new(
                                                    egui::RichText::new(format!("{}", i + 1)).size(9.0)
                                                        .color(if selected { egui::Color32::BLACK } else { egui::Color32::WHITE })
                                                )
                                                .fill(if selected { egui::Color32::from_rgb(0, 180, 216) } else { egui::Color32::from_rgb(30, 30, 40) })
                                                .corner_radius(4)
                                                .min_size(egui::vec2(20.0, 20.0));
                                                if ui.add(btn).clicked() {
                                                    setter.set_parameter(&data.params.snapshot, i as i32);
                                                }
                                            }
                                        });
                                    });
                                });

                            ui.add_space(10.0);
                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(14, 13, 21))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 37, 52)))
                                .inner_margin(8.0)
                                .corner_radius(6)
                                .show(ui, |ui| {
                                    let m = &data.dsp_metrics;
                                    let load = m.process_time_ns.load(std::sync::atomic::Ordering::Relaxed);
                                    let buf = m.buffer_size.load(std::sync::atomic::Ordering::Relaxed);
                                    let sr = m.sample_rate.load(std::sync::atomic::Ordering::Relaxed);
                                    ui.label(egui::RichText::new(format!("CPU: {} µs  |  {} @ {} Hz", load / 100, buf, sr / 1000)).size(8.0).color(egui::Color32::GRAY));
                                });
                        }
                    );
                });
            });
        },
    )
}
