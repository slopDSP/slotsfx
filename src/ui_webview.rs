//! Webview-based editor for SlotsFX.
//!
//! Spawns `wry` directly as a child of the DAW's window handle on the main GUI thread.
//! This avoids cross-thread window parenting errors and is natively cross-platform.

use std::sync::Arc;
use std::sync::Mutex;
use std::any::Any;
use std::borrow::Cow;

use nih_plug::prelude::*;
use serde::Deserialize;
use include_dir::{include_dir, Dir};

use crate::ui::SlotsEditorData;
use crate::SlotsFxParams;

#[cfg(target_os = "windows")]
use tao::platform::windows::EventLoopBuilderExtWindows;

/// Embed the built Vite UI at compile time.
static DIST: Dir = include_dir!("$CARGO_MANIFEST_DIR/ui_web/dist");

/// Map a file path to its MIME type.
fn mime_from_path(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum IpcMessage {
    #[serde(rename = "set_param")]
    SetParam { param_id: String, value: f32 },

    #[serde(rename = "set_bypass")]
    SetBypass { param_id: String, value: bool },

    #[serde(rename = "load_nam")]
    LoadNam { slot_id: String, filename: Option<String> },

    #[serde(rename = "load_cab")]
    LoadCab { slot_id: String, filename: Option<String> },

    #[serde(rename = "prev_file")]
    PrevFile { slot_id: String, slot: String, current_path: Option<String> },

    #[serde(rename = "next_file")]
    NextFile { slot_id: String, slot: String, current_path: Option<String> },

    #[serde(rename = "profile_captured")]
    ProfileCaptured { slot_id: String, ir_name: String },

    #[serde(rename = "ui_ready")]
    UiReady,

    #[serde(rename = "get_metrics")]
    GetMetrics,

    #[serde(rename = "update_slots")]
    UpdateSlots { slots: Vec<serde_json::Value> },

    #[serde(rename = "save_snapshots")]
    SaveSnapshots { snapshots: Vec<serde_json::Value> },
}

/// Custom dummy window wrapping raw win32 HWND and implementing HasWindowHandle/HasDisplayHandle
struct RawWindow {
    hwnd: *mut std::ffi::c_void,
}

impl wry::raw_window_handle::HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<wry::raw_window_handle::WindowHandle<'_>, wry::raw_window_handle::HandleError> {
        let hwnd_val = std::num::NonZeroIsize::new(self.hwnd as isize).unwrap();
        let win32_handle = wry::raw_window_handle::Win32WindowHandle::new(hwnd_val);
        let raw_handle = wry::raw_window_handle::RawWindowHandle::Win32(win32_handle);
        unsafe { Ok(wry::raw_window_handle::WindowHandle::borrow_raw(raw_handle)) }
    }
}

impl wry::raw_window_handle::HasDisplayHandle for RawWindow {
    fn display_handle(&self) -> Result<wry::raw_window_handle::DisplayHandle<'_>, wry::raw_window_handle::HandleError> {
        let windows_handle = wry::raw_window_handle::WindowsDisplayHandle::new();
        let raw_display = wry::raw_window_handle::RawDisplayHandle::Windows(windows_handle);
        unsafe { Ok(wry::raw_window_handle::DisplayHandle::borrow_raw(raw_display)) }
    }
}

pub struct SafeWebView(pub wry::WebView);
unsafe impl Send for SafeWebView {}
unsafe impl Sync for SafeWebView {}

/// Handle for the webview editor. Dropping it destroys the WebView.
pub struct WebviewEditorHandle {
    _webview: Arc<Mutex<Option<SafeWebView>>>,
    _thread: Option<std::thread::JoinHandle<()>>,
    close_tx: Option<std::sync::mpsc::Sender<()>>,
}

unsafe impl Send for WebviewEditorHandle {}

impl Drop for WebviewEditorHandle {
    fn drop(&mut self) {
        if let Some(close_tx) = self.close_tx.take() {
            let _ = close_tx.send(());
        }
    }
}

/// nih-plug Editor implementation for the webview UI.
pub struct WebviewEditor {
    data: SlotsEditorData,
    params: Arc<SlotsFxParams>,
}

impl WebviewEditor {
    pub fn new(data: SlotsEditorData, params: Arc<SlotsFxParams>) -> Self {
        Self { data, params }
    }
}

impl Editor for WebviewEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        // Extract raw parent HWND
        let hwnd = match parent {
            ParentWindowHandle::Win32Hwnd(h) => h,
            _ => panic!("Unsupported platform (only Windows Win32 currently configured)"),
        };
        let parent_window = RawWindow { hwnd };

        // Setup temporary User Data Directory for WebView2 context
        let mut webview_data_dir = std::env::temp_dir();
        webview_data_dir.push("slotsfx-webview-data");
        let _ = std::fs::create_dir_all(&webview_data_dir);
        let mut web_context = wry::WebContext::new(Some(webview_data_dir));

        // Setup asset serving custom protocol
        let protocol_handler = move |request: http::Request<Vec<u8>>| {
            let path = request.uri().path();
            let mut clean_path = path.trim_start_matches('/').to_string();
            if clean_path.is_empty() || clean_path == "/" {
                clean_path = "index.html".to_string();
            }

            if clean_path.contains("..") {
                return http::Response::builder()
                    .status(403)
                    .header("Content-Type", "text/plain")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Cow::Borrowed(b"Forbidden" as &[u8]))
                    .unwrap();
            }

            if let Some(file) = DIST.get_file(&clean_path) {
                let mime = mime_from_path(&clean_path);
                let contents = file.contents();
                http::Response::builder()
                    .status(200)
                    .header("Content-Type", mime)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Cow::Owned(contents.to_vec()))
                    .unwrap()
            } else {
                let msg = format!(
                    "<!doctype html><html><body style=\"background:#111;color:#fff;font-family:system-ui,sans-serif;padding:24px;\"><h1>Missing embedded asset</h1><p>Path: {}</p></body></html>",
                    clean_path
                );
                http::Response::builder()
                    .status(404)
                    .header("Content-Type", "text/html")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Cow::Owned(msg.into_bytes()))
                    .unwrap()
            }
        };

        // Capture clones for the IPC callback closure
        let context_clone = context.clone();
        let params_clone = self.params.clone();
        let data_clone = self.data.clone();

        // Create holder to share WebView pointer safely
        let webview_holder = Arc::new(Mutex::new(None::<SafeWebView>));
        let webview_weak = Arc::downgrade(&webview_holder);

        // Build the webview directly inside the parent window
        let webview_weak_clone = webview_weak.clone();
        let webview = wry::WebViewBuilder::new(&parent_window)
            .with_web_context(&mut web_context)
            .with_custom_protocol("slotsfx".to_string(), protocol_handler)
            .with_url("slotsfx://localhost/index.html")
            .with_ipc_handler(move |request: http::Request<String>| {
                let msg = request.into_body();
                if let Ok(ipc) = serde_json::from_str::<IpcMessage>(&msg) {
                    match ipc {
                        IpcMessage::SetParam { param_id, value } => {
                            let setter = ParamSetter::new(&*context_clone);
                            match param_id.as_str() {
                                "input_gain" | "input" => setter.set_parameter(&params_clone.input_gain, value),
                                "output_gain" | "output" => setter.set_parameter(&params_clone.output_gain, value),
                                "mix" => setter.set_parameter(&params_clone.mix, value),

                                // Amp / NAM
                                "amp_gain" | "nam_gain" | "gain" => setter.set_parameter(&params_clone.nam_gain, value),
                                "amp_bass" | "bass" => setter.set_parameter(&params_clone.amp_bass, value),
                                "amp_middle" | "middle" | "mid" => setter.set_parameter(&params_clone.amp_middle, value),
                                "amp_high" | "high" | "treble" => setter.set_parameter(&params_clone.amp_high, value),
                                "amp_output" | "amp_out" => setter.set_parameter(&params_clone.amp_output, value),
                                "amp_bass_freq" | "bass_freq" => setter.set_parameter(&params_clone.amp_bass_freq, value),
                                "amp_mid_freq" | "mid_freq" => setter.set_parameter(&params_clone.amp_mid_freq, value),
                                "amp_high_freq" | "high_freq" => setter.set_parameter(&params_clone.amp_high_freq, value),

                                // Cab
                                "cab_gain" => setter.set_parameter(&params_clone.cab_gain, value),
                                "cab_position" | "cab_pos" | "position" | "pos" => setter.set_parameter(&params_clone.cab_position, value),
                                "cab_size" | "size" => setter.set_parameter(&params_clone.cab_size, value),
                                "cab_normalize" => setter.set_parameter(&params_clone.cab_normalize, value > 0.5),

                                // Pitch
                                "pitch_gain" => setter.set_parameter(&params_clone.pitch_gain, value),
                                "pitch_semi" | "semi" => setter.set_parameter(&params_clone.pitch_semi, value),
                                "pitch_mix" => setter.set_parameter(&params_clone.pitch_mix, value),

                                // Auto-Tune
                                "auto_tune_toggle" => setter.set_parameter(&params_clone.auto_tune_toggle, value > 0.5),
                                "auto_tune_key" => setter.set_parameter(&params_clone.auto_tune_key, value as i32),
                                "auto_tune_scale" => setter.set_parameter(&params_clone.auto_tune_scale, value as i32),
                                "auto_tune_mode" => setter.set_parameter(&params_clone.auto_tune_mode, value > 0.5),
                                "auto_tune_speed" => setter.set_parameter(&params_clone.auto_tune_speed, value),
                                "auto_tune_amount" => setter.set_parameter(&params_clone.auto_tune_amount, value),

                                // Delay
                                "delay_mix" => setter.set_parameter(&params_clone.delay_mix, value),
                                "delay_feedback" | "feedback" | "fdbk" => setter.set_parameter(&params_clone.delay_feedback, value),
                                "delay_time" | "time" => setter.set_parameter(&params_clone.delay_time, value),
                                "delay_ducking" => setter.set_parameter(&params_clone.delay_ducking, value),
                                "delay_ping_pong" | "ping_pong" => setter.set_parameter(&params_clone.delay_ping_pong, value > 0.5),

                                // Reverb
                                "reverb_mix" | "verb_mix" | "shimmer_mix" => setter.set_parameter(&params_clone.reverb_mix, value),
                                "reverb_space" | "verb_space" | "space" => setter.set_parameter(&params_clone.reverb_space, value),
                                "reverb_shimmer" | "shimmer" => setter.set_parameter(&params_clone.reverb_shimmer, value),
                                "reverb_ducking" => setter.set_parameter(&params_clone.reverb_ducking, value),

                                // Gate
                                "gate_threshold" | "threshold" | "thresh" => setter.set_parameter(&params_clone.gate_threshold, value),
                                "gate_attack" | "attack" | "atk" => setter.set_parameter(&params_clone.gate_attack, value),
                                "gate_release" | "release" | "rel" => setter.set_parameter(&params_clone.gate_release, value),

                                // Bitcrusher
                                "bitcrush_bits" | "bits" => setter.set_parameter(&params_clone.bitcrush_bits, value),
                                "bitcrush_downsample" | "downsample" | "down" => setter.set_parameter(&params_clone.bitcrush_downsample, value),
                                "bitcrush_mix" => setter.set_parameter(&params_clone.bitcrush_mix, value),
                                "bitcrush_mode" | "mode" => setter.set_parameter(&params_clone.bitcrush_mode, value),

                                // Overdrive
                                "overdrive_drive" | "drive" => setter.set_parameter(&params_clone.overdrive_drive, value),
                                "overdrive_tone" | "tone" => setter.set_parameter(&params_clone.overdrive_tone, value),
                                "overdrive_level" | "level" => setter.set_parameter(&params_clone.overdrive_level, value),
                                "overdrive_algo" | "algo" => setter.set_parameter(&params_clone.overdrive_algo, value),

                                // EQ
                                "eq_low_freq" => setter.set_parameter(&params_clone.eq_low_freq, value),
                                "eq_low_gain" | "low_gain" | "log" => setter.set_parameter(&params_clone.eq_low_gain, value),
                                "eq_mid_freq" => setter.set_parameter(&params_clone.eq_mid_freq, value),
                                "eq_mid_gain" | "mid_gain" | "midg" => setter.set_parameter(&params_clone.eq_mid_gain, value),
                                "eq_mid_q" => setter.set_parameter(&params_clone.eq_mid_q, value),
                                "eq_high_freq" => setter.set_parameter(&params_clone.eq_high_freq, value),
                                "eq_high_gain" | "high_gain" | "hig" => setter.set_parameter(&params_clone.eq_high_gain, value),

                                // Amp normalize (float, not bool, so it's a plain set)
                                "amp_normalize" => setter.set_parameter(&params_clone.amp_normalize, value > 0.5),

                                // Snapshot
                                "snapshot" => setter.set_parameter(&params_clone.snapshot, value as i32),

                                // Macros
                                "macro_1" => setter.set_parameter(&params_clone.macro_1, value),
                                "macro_2" => setter.set_parameter(&params_clone.macro_2, value),
                                "macro_3" => setter.set_parameter(&params_clone.macro_3, value),
                                "macro_4" => setter.set_parameter(&params_clone.macro_4, value),
                                _ => {}
                            }
                        }
                        IpcMessage::SetBypass { param_id, value } => {
                            let setter = ParamSetter::new(&*context_clone);
                            match param_id.as_str() {
                                "amp_bypass" | "nam_bypass" => setter.set_parameter(&params_clone.nam_bypass, value),
                                "cab_bypass" => setter.set_parameter(&params_clone.cab_bypass, value),
                                "pitch_bypass" => setter.set_parameter(&params_clone.pitch_bypass, value),
                                "delay_bypass" => setter.set_parameter(&params_clone.delay_bypass, value),
                                "reverb_bypass" | "shimmer_bypass" | "verb_bypass" => setter.set_parameter(&params_clone.reverb_bypass, value),
                                "gate_bypass" => setter.set_parameter(&params_clone.gate_bypass, value),
                                // These are BoolParams but the JS sends them via set_bypass
                                "amp_normalize" => setter.set_parameter(&params_clone.amp_normalize, value),
                                "cab_normalize" => {
                                    setter.set_parameter(&params_clone.cab_normalize, value);
                                    // Toggle normalization in the convolver without reloading IR
                                    if let Some(ref mut sender) = *data_clone.cab_normalize_sender.lock().unwrap() {
                                        let _ = sender.push((value, None, None));
                                    }
                                }
                                _ => {}
                            }
                        }
                        IpcMessage::LoadNam { slot_id, filename } => {
                            if let Some(name) = filename {
                                // Resolve path inside C:\LIBRARIES\NAM MODELS\DIEZEL VH4
                                let mut path = std::path::PathBuf::from(r"C:\LIBRARIES\NAM MODELS\DIEZEL VH4");
                                path.push(&name);

                                if path.exists() {
                                    if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                        if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                            let script = format!(
                                                "if (window.onFileLoaded) window.onFileLoaded('nam', '{}', '{}', '{}');",
                                                name.replace('\\', "\\\\").replace('\'', "\\'"),
                                                slot_id,
                                                path.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'")
                                            );
                                            let _ = wv.0.evaluate_script(&script);
                                        }
                                    }
                                }
                            } else {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("NAM Model", &["nam"])
                                    .pick_file()
                                {
                                    let display_name = path
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned();

                                    if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                        if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                            let script = format!(
                                                "if (window.onFileLoaded) window.onFileLoaded('nam', '{}', '{}', '{}');",
                                                display_name.replace('\\', "\\\\").replace('\'', "\\'"),
                                                slot_id,
                                                path.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'")
                                            );
                                            let _ = wv.0.evaluate_script(&script);
                                        }
                                    }
                                }
                            }
                        }
                        IpcMessage::LoadCab { slot_id, filename } => {
                            if let Some(name) = filename {
                                // Resolve path inside C:\LIBRARIES\IR
                                let mut path = std::path::PathBuf::from(r"C:\LIBRARIES\IR");
                                path.push(&name);

                                if path.exists() {
                                    if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                        if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                            let script = format!(
                                                "if (window.onFileLoaded) window.onFileLoaded('cab', '{}', '{}', '{}');",
                                                name.replace('\\', "\\\\").replace('\'', "\\'"),
                                                slot_id,
                                                path.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'")
                                            );
                                            let _ = wv.0.evaluate_script(&script);
                                        }
                                    }
                                }
                            } else {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("WAV Audio", &["wav"])
                                    .pick_file()
                                {
                                    let display_name = path
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned();

                                    if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                        if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                            let script = format!(
                                                "if (window.onFileLoaded) window.onFileLoaded('cab', '{}', '{}', '{}');",
                                                display_name.replace('\\', "\\\\").replace('\'', "\\'"),
                                                slot_id,
                                                path.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'")
                                            );
                                            let _ = wv.0.evaluate_script(&script);
                                        }
                                    }
                                }
                            }
                        }
                        IpcMessage::PrevFile { slot_id, slot, current_path } => {
                            let path_opt = current_path.map(std::path::PathBuf::from);
                            switch_file(&slot, -1, &slot_id, path_opt, webview_weak_clone.clone());
                        }
                        IpcMessage::NextFile { slot_id, slot, current_path } => {
                            let path_opt = current_path.map(std::path::PathBuf::from);
                            switch_file(&slot, 1, &slot_id, path_opt, webview_weak_clone.clone());
                        }
                        IpcMessage::ProfileCaptured { slot_id: _, ir_name: _ } => {
                            // Empty profile hook callback
                        }
                        IpcMessage::UiReady => {
                            let slots_json = params_clone.slots_json.lock().unwrap().clone();
                            let snapshots_json = params_clone.snapshots_json.lock().unwrap().clone();
                            if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                    fn escape_json(s: &str) -> String {
                                        s.replace('\\', "\\\\").replace('\'', "\\'").replace('"', "\\\"")
                                    }
                                    let script = format!(
                                        "if (window.syncSlots) window.syncSlots('{}'); if (window.syncSnapshots) window.syncSnapshots('{}');",
                                        escape_json(&slots_json),
                                        escape_json(&snapshots_json)
                                    );
                                    let _ = wv.0.evaluate_script(&script);
                                }
                            }
                        }
                        IpcMessage::GetMetrics => {
                            if let Some(wv_shared) = webview_weak_clone.upgrade() {
                                if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                    use std::sync::atomic::Ordering;
                                    let proc_time = data_clone.dsp_metrics.process_time_ns.load(Ordering::Relaxed);
                                    let block_dur = data_clone.dsp_metrics.block_duration_ns.load(Ordering::Relaxed);
                                    let nam_time = data_clone.dsp_metrics.nam_time_ns.load(Ordering::Relaxed);
                                    let cab_time = data_clone.dsp_metrics.cab_time_ns.load(Ordering::Relaxed);
                                    let peak_bits = data_clone.dsp_metrics.peak_level_bits.load(Ordering::Relaxed);
                                    let peak_val = f32::from_bits(peak_bits);
                                    let input_peak_bits = data_clone.dsp_metrics.input_peak_level_bits.load(Ordering::Relaxed);
                                    let input_peak_val = f32::from_bits(input_peak_bits);
                                    let buf_size = data_clone.dsp_metrics.buffer_size.load(Ordering::Relaxed);
                                    let s_rate = data_clone.dsp_metrics.sample_rate.load(Ordering::Relaxed);
                                    let tuner_packed = data_clone.dsp_metrics.tuner_state.load(Ordering::Relaxed);
                                    let tuner_active = (tuner_packed >> 8) & 0xFF;
                                    let tuner_cents_raw = if tuner_active != 0 {
                                        ((tuner_packed >> 32) & 0xFFFF) as i16
                                    } else { 0 };
                                    let tuner_note = ((tuner_packed >> 24) & 0xFF) as u8;
                                    let tuner_octave = ((tuner_packed >> 16) & 0xFF) as i8;

                                    let tuner_note_val: i32 = if tuner_active != 0 { tuner_note as i32 } else { -1 };
                                    let tuner_octave_val: i32 = if tuner_active != 0 { tuner_octave as i32 } else { 0 };
                                    let tuner_cents_val: f64 = if tuner_active != 0 { tuner_cents_raw as f64 / 100.0 } else { 0.0 };

                                    let script = format!(
                                        "if (window.updateDspMetrics) window.updateDspMetrics({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {});",
                                        proc_time, block_dur, nam_time, cab_time, peak_val, buf_size, s_rate, input_peak_val,
                                        tuner_note_val, tuner_octave_val, tuner_cents_val
                                    );
                                    let _ = wv.0.evaluate_script(&script);
                                }
                            }
                        }
                        IpcMessage::UpdateSlots { slots } => {
                            // Build a Vec<SlotConfig> from the JSON and hand it to the audio
                            // thread via the existing blocks_sender queue. NAM/cab IR loading
                            // happens in the processor (see PluginProcessor::update_from_configs).
                            let mut nam_to_load = None;
                            let mut cab_to_load = None;

                            let new_configs: Vec<crate::dsp::SlotConfig> = slots
                                .iter()
                                .map(|s| {
                                    let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let slot_type = s.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let path = s.get("path").and_then(|v| v.as_str()).map(std::path::PathBuf::from);
                                    let bypassed = s.get("bypassed").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let pan = s.get("pan").and_then(|v| v.as_f64()).map(|f| f as f32).unwrap_or(0.0);
                                    let lane = s.get("lane").and_then(|v| v.as_str()).unwrap_or("serial").to_string();
                                    
                                    if slot_type == "amp" && nam_to_load.is_none() {
                                        if let Some(ref p) = path {
                                            nam_to_load = Some(p.clone());
                                        }
                                    } else if slot_type == "cab" && cab_to_load.is_none() {
                                        if let Some(ref p) = path {
                                            cab_to_load = Some(p.clone());
                                        }
                                    }

                                    let params_map = s.get("params").and_then(|v| v.as_object()).map(|obj| {
                                        obj.iter()
                                            .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f as f32)))
                                            .collect::<std::collections::HashMap<String, f32>>()
                                    }).unwrap_or_default();
                                    crate::dsp::SlotConfig {
                                        id,
                                        slot_type,
                                        name,
                                        path,
                                        bypassed,
                                        pan,
                                        lane,
                                        params: params_map,
                                    }
                                })
                                .collect();

                            // Persist the state JSON string
                            if let Ok(slots_str) = serde_json::to_string(&slots) {
                                *params_clone.slots_json.lock().unwrap() = slots_str;
                            }

                            // Push new configs to audio thread
                            if let Some(ref mut sender) = *data_clone.blocks_sender.lock().unwrap() {
                                let _ = sender.push(new_configs);
                            }

                            // Reload NAM
                            if let Some(p) = nam_to_load {
                                let mut loaded = false;
                                if let Some(model_file) = params_clone.nam_cache.lock().unwrap().get(&p) {
                                    let loudness = model_file.loudness();
                                    if let (Ok(ml), Ok(mr)) = (
                                        nam_rs::Model::from_nam(model_file),
                                        nam_rs::Model::from_nam(model_file),
                                    ) {
                                        if let Some(ref mut sender) = *data_clone.nam_sender.lock().unwrap() {
                                            let _ = sender.push((ml, mr, loudness));
                                            loaded = true;
                                        }
                                    }
                                }
                                if !loaded && p.exists() {
                                    if let Ok(model_file) = nam_rs::NamModel::from_file(&p) {
                                        let loudness = model_file.loudness();
                                        if let (Ok(ml), Ok(mr)) = (
                                            nam_rs::Model::from_nam(&model_file),
                                            nam_rs::Model::from_nam(&model_file),
                                        ) {
                                            params_clone.nam_cache.lock().unwrap().insert(p.clone(), std::sync::Arc::new(model_file));
                                            if let Some(ref mut sender) = *data_clone.nam_sender.lock().unwrap() {
                                                let _ = sender.push((ml, mr, loudness));
                                            }
                                        }
                                    }
                                }
                            }

                            // Reload CAB
                            if let Some(p) = cab_to_load {
                                let mut loaded = false;
                                if let Some((ir_l, ir_r)) = params_clone.cab_cache.lock().unwrap().get(&p) {
                                    if let Some(ref mut sender) = *data_clone.cab_sender.lock().unwrap() {
                                        let _ = sender.push((ir_l.clone(), ir_r.clone()));
                                        loaded = true;
                                    }
                                }
                                if !loaded && p.exists() {
                                    if let Ok(mut reader) = hound::WavReader::open(&p) {
                                        let spec = reader.spec();
                                        let samples: Vec<f32> = match spec.sample_format {
                                            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
                                            hound::SampleFormat::Int => {
                                                let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
                                                reader.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / max_val).collect()
                                            }
                                        };
                                        if !samples.is_empty() {
                                            let (ir_l, ir_r) = if spec.channels == 2 {
                                                (
                                                    samples.iter().step_by(2).copied().collect(),
                                                    samples.iter().skip(1).step_by(2).copied().collect(),
                                                )
                                            } else {
                                                (samples.clone(), samples.clone())
                                            };
                                            params_clone.cab_cache.lock().unwrap().insert(p.clone(), (ir_l.clone(), ir_r.clone()));
                                            if let Some(ref mut sender) = *data_clone.cab_sender.lock().unwrap() {
                                                let _ = sender.push((ir_l, ir_r));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        IpcMessage::SaveSnapshots { snapshots } => {
                            let json_str = serde_json::to_string(&snapshots).unwrap_or_default();
                            *params_clone.snapshots_json.lock().unwrap() = json_str;
                            params_clone.rt_snapshots.store(Arc::new(snapshots));
                        }
                    }
                }
            })
            .build()
            .expect("[SlotsFX] WebView creation failed");
        *webview_holder.lock().unwrap() = Some(SafeWebView(webview));

        Box::new(WebviewEditorHandle {
            _webview: webview_holder,
            _thread: None,
            close_tx: None,
        })
    }

    fn size(&self) -> (u32, u32) {
        (740, 520)
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
    fn param_values_changed(&self) {}
}

/// Entry point to create the webview editor.
pub fn create_webview_editor(
    data: SlotsEditorData,
    params: Arc<SlotsFxParams>,
) -> Option<Box<dyn Editor>> {
    Some(Box::new(WebviewEditor::new(data, params)))
}

/// Switches the loaded model or impulse response to the previous or next file alphabetically
/// in the same directory as the currently loaded file.
fn switch_file(
    slot: &str,
    direction: i32,
    slot_id: &str,
    current_path_opt: Option<std::path::PathBuf>,
    webview_weak: std::sync::Weak<Mutex<Option<SafeWebView>>>,
) {
    if let Some(current_path) = current_path_opt {
        if let Some(parent_dir) = current_path.parent() {
            let filter_ext = if slot == "nam" { "nam" } else { "wav" };
            if let Ok(entries) = std::fs::read_dir(parent_dir) {
                let mut files: Vec<std::path::PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file()
                            && p.extension()
                                .map_or(false, |ext| ext.eq_ignore_ascii_case(filter_ext))
                    })
                    .collect();

                files.sort();

                if !files.is_empty() {
                    if let Some(current_idx) = files.iter().position(|p| p == &current_path) {
                        let count = files.len() as i32;
                        let new_idx = ((current_idx as i32 + direction) % count + count) % count;
                        let next_path = &files[new_idx as usize];
                        let display_name = next_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();

                        // Notify Webview
                        if let Some(wv_shared) = webview_weak.upgrade() {
                            if let Some(ref wv) = *wv_shared.lock().unwrap() {
                                let script = format!(
                                    "if (window.onFileLoaded) window.onFileLoaded('{}', '{}', '{}', '{}');",
                                    slot,
                                    display_name.replace('\\', "\\\\").replace('\'', "\\'"),
                                    slot_id,
                                    next_path.to_string_lossy().replace('\\', "\\\\").replace('\'', "\\'")
                                );
                                let _ = wv.0.evaluate_script(&script);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Spawn the embedded webview in a standalone window for testing.
pub fn spawn_debug_webview() -> WebviewEditorHandle {
    let (close_tx, close_rx) = std::sync::mpsc::channel();

    let webview_thread = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut webview_data_dir = std::env::temp_dir();
            webview_data_dir.push("slotsfx-webview-data");
            let _ = std::fs::create_dir_all(&webview_data_dir);
            let mut web_context = wry::WebContext::new(Some(webview_data_dir));

            let url = if DIST.entries().is_empty() {
                "data:text/html,<h1>UI not built — run npm run build in ui_web/ then cargo build</h1>".to_string()
            } else {
                "slotsfx://localhost/index.html".to_string()
            };

            let protocol_handler = move |request: http::Request<Vec<u8>>| {
                let path = request.uri().path();
                let mut clean_path = path.trim_start_matches('/').to_string();
                if clean_path.is_empty() || clean_path == "/" {
                    clean_path = "index.html".to_string();
                }

                if clean_path.contains("..") {
                    return http::Response::builder()
                        .status(403)
                        .header("Content-Type", "text/plain")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Cow::Borrowed(b"Forbidden" as &[u8]))
                        .unwrap();
                }

                if let Some(file) = DIST.get_file(&clean_path) {
                    let mime = mime_from_path(&clean_path);
                    let contents = file.contents();
                    http::Response::builder()
                        .status(200)
                        .header("Content-Type", mime)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Cow::Owned(contents.to_vec()))
                        .unwrap()
                } else {
                    let msg = format!(
                        "<!doctype html><html><body style=\"background:#111;color:#fff;font-family:system-ui,sans-serif;padding:24px;\"><h1>Missing embedded asset</h1><p>Path: {}</p></body></html>",
                        clean_path
                    );
                    http::Response::builder()
                        .status(404)
                        .header("Content-Type", "text/html")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Cow::Owned(msg.into_bytes()))
                        .unwrap()
                }
            };

            let event_loop = tao::event_loop::EventLoopBuilder::new().with_any_thread(true).build();
            let window = tao::window::WindowBuilder::new()
                .with_title("SlotsFX Debug Webview")
                .with_inner_size(tao::dpi::LogicalSize::new(740.0, 520.0))
                .build(&event_loop)
                .expect("Failed to create window");

            let _webview = wry::WebViewBuilder::new(&window)
                .with_web_context(&mut web_context)
                .with_url(&url)
                .with_custom_protocol("slotsfx".to_string(), protocol_handler)
                .build()
                .expect("WebView creation failed");

            event_loop.run(move |_event, _event_loop, control_flow| {
                *control_flow = tao::event_loop::ControlFlow::Wait;
                if close_rx.try_recv().is_ok() {
                    *control_flow = tao::event_loop::ControlFlow::Exit;
                }
            });
        }));

        if let Err(err) = result {
            eprintln!("debug webview thread panicked: {:?}", err);
        }
    });

    WebviewEditorHandle {
        _webview: Arc::new(Mutex::new(None)),
        _thread: Some(webview_thread),
        close_tx: Some(close_tx),
    }
}
