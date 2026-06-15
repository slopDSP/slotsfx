use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;
use std::sync::Mutex;
use rtrb::RingBuffer;
use dsp::PluginProcessor;
use crate::dsp::EffectBlock;
use crate::dsp::nam::NamBlock;
use crate::dsp::cab::CabConvolver;
use std::collections::HashMap;
use crate::ui::SlotsEditorData;

mod dsp;
mod ui;
mod ui_webview;
mod ui_egui;
mod registry;

pub struct DspMetrics {
    pub process_time_ns: std::sync::atomic::AtomicU32,
    pub block_duration_ns: std::sync::atomic::AtomicU32,
    pub nam_time_ns: std::sync::atomic::AtomicU32,
    pub cab_time_ns: std::sync::atomic::AtomicU32,
    pub peak_level_bits: std::sync::atomic::AtomicU32,
    pub input_peak_level_bits: std::sync::atomic::AtomicU32,
    pub slot_peaks: [std::sync::atomic::AtomicU32; 16],
    pub buffer_size: std::sync::atomic::AtomicU32,
    pub sample_rate: std::sync::atomic::AtomicU32,
}

impl DspMetrics {
    pub fn new() -> Self {
        Self {
            process_time_ns: std::sync::atomic::AtomicU32::new(0),
            block_duration_ns: std::sync::atomic::AtomicU32::new(0),
            nam_time_ns: std::sync::atomic::AtomicU32::new(0),
            cab_time_ns: std::sync::atomic::AtomicU32::new(0),
            peak_level_bits: std::sync::atomic::AtomicU32::new(0.0f32.to_bits()),
            input_peak_level_bits: std::sync::atomic::AtomicU32::new(0.0f32.to_bits()),
            slot_peaks: [
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ],
            buffer_size: std::sync::atomic::AtomicU32::new(0),
            sample_rate: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

pub struct SlotsFx {
    params: Arc<SlotsFxParams>,
    processor: PluginProcessor,
    dsp_metrics: Arc<DspMetrics>,
    
    blocks_receiver: rtrb::Consumer<Vec<crate::dsp::SlotConfig>>,
    blocks_sender: Arc<Mutex<Option<rtrb::Producer<Vec<crate::dsp::SlotConfig>>>>>,

    nam_models_receiver: rtrb::Consumer<(nam_rs::Model, nam_rs::Model, Option<f32>)>,
    nam_sender: Arc<Mutex<Option<rtrb::Producer<(nam_rs::Model, nam_rs::Model, Option<f32>)>>>>,

    cab_irs_receiver: rtrb::Consumer<(Vec<f32>, Vec<f32>)>,
    cab_sender: Arc<Mutex<Option<rtrb::Producer<(Vec<f32>, Vec<f32>)>>>>,

    /// Receiver for cab normalize toggle: (normalize: bool, Option<ir_l>, Option<ir_r>)
    /// When ir_l/ir_r are Some, IR is reloaded. When None, only normalize flag is toggled.
    cab_normalize_receiver: rtrb::Consumer<(bool, Option<Vec<f32>>, Option<Vec<f32>>)>,
    cab_normalize_sender: Arc<Mutex<Option<rtrb::Producer<(bool, Option<Vec<f32>>, Option<Vec<f32>>)>>>>,

    routing_receiver: rtrb::Consumer<[usize; 5]>,
    routing_sender: Arc<Mutex<Option<rtrb::Producer<[usize; 5]>>>>,

    nam_model_name: Arc<Mutex<String>>,
    nam_model_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    cab_ir_name: Arc<Mutex<String>>,
    cab_ir_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    routing_order: Arc<Mutex<[usize; 5]>>,

    dry_left_buffer: Vec<f32>,
    dry_right_buffer: Vec<f32>,

    instance_id: usize,
    shared_data: Arc<registry::InstanceSharedData>,
    sender_ring: Option<rtrb::Producer<f32>>,
    receiver_ring: Option<rtrb::Consumer<f32>>,
    sweep_sample_counter: u32,
    
    capture_active: bool,
    capture_latency_gate: bool,
    capture_buffer: Vec<f32>,
    capture_write_ptr: usize,
}

#[derive(Params)]
pub struct SlotsFxParams {
    #[id = "input_gain"]
    pub input_gain: FloatParam,

    #[id = "output_gain"]
    pub output_gain: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "pitch_gain"]
    pub pitch_gain: FloatParam,

    #[id = "nam_gain"]
    pub nam_gain: FloatParam,

    #[id = "cab_gain"]
    pub cab_gain: FloatParam,

    #[id = "delay_mix"]
    pub delay_mix: FloatParam,

    #[id = "delay_feedback"]
    pub delay_feedback: FloatParam,

    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,

    #[id = "reverb_space"]
    pub reverb_space: FloatParam,

    #[id = "pitch_bypass"]
    pub pitch_bypass: BoolParam,

    #[id = "nam_bypass"]
    pub nam_bypass: BoolParam,

    #[id = "cab_bypass"]
    pub cab_bypass: BoolParam,

    #[id = "delay_bypass"]
    pub delay_bypass: BoolParam,

    #[id = "reverb_bypass"]
    pub reverb_bypass: BoolParam,

    // --- Amp EQ ---
    #[id = "amp_bass"]
    pub amp_bass: FloatParam,

    #[id = "amp_middle"]
    pub amp_middle: FloatParam,

    #[id = "amp_high"]
    pub amp_high: FloatParam,

    #[id = "amp_output"]
    pub amp_output: FloatParam,

    // --- Amp EQ Frequencies ---
    #[id = "amp_bass_freq"]
    pub amp_bass_freq: FloatParam,

    #[id = "amp_mid_freq"]
    pub amp_mid_freq: FloatParam,

    #[id = "amp_high_freq"]
    pub amp_high_freq: FloatParam,

    // --- Noise Gate ---
    #[id = "gate_bypass"]
    pub gate_bypass: BoolParam,

    #[id = "gate_threshold"]
    pub gate_threshold: FloatParam,

    #[id = "gate_attack"]
    pub gate_attack: FloatParam,

    #[id = "gate_release"]
    pub gate_release: FloatParam,

    // --- Cab position/size ---
    #[id = "cab_position"]
    pub cab_position: FloatParam,

    #[id = "cab_size"]
    pub cab_size: FloatParam,

    // --- Pitch semi/mix ---
    #[id = "pitch_semi"]
    pub pitch_semi: FloatParam,

    #[id = "pitch_mix"]
    pub pitch_mix: FloatParam,

    // --- Delay time ---
    #[id = "delay_time"]
    pub delay_time: FloatParam,

    // --- Reverb shimmer ---
    #[id = "reverb_shimmer"]
    pub reverb_shimmer: FloatParam,

    // --- Bitcrusher ---
    #[id = "bitcrush_bits"]
    pub bitcrush_bits: FloatParam,

    #[id = "bitcrush_downsample"]
    pub bitcrush_downsample: FloatParam,

    #[id = "bitcrush_mix"]
    pub bitcrush_mix: FloatParam,

    #[id = "bitcrush_mode"]
    pub bitcrush_mode: FloatParam,

    // --- Overdrive ---
    #[id = "overdrive_drive"]
    pub overdrive_drive: FloatParam,

    #[id = "overdrive_tone"]
    pub overdrive_tone: FloatParam,

    #[id = "overdrive_level"]
    pub overdrive_level: FloatParam,

    #[id = "overdrive_algo"]
    pub overdrive_algo: FloatParam,

    // --- EQ ---
    #[id = "eq_low_freq"]
    pub eq_low_freq: FloatParam,

    #[id = "eq_low_gain"]
    pub eq_low_gain: FloatParam,

    #[id = "eq_mid_freq"]
    pub eq_mid_freq: FloatParam,

    #[id = "eq_mid_gain"]
    pub eq_mid_gain: FloatParam,

    #[id = "eq_mid_q"]
    pub eq_mid_q: FloatParam,

    #[id = "eq_high_freq"]
    pub eq_high_freq: FloatParam,

    #[id = "eq_high_gain"]
    pub eq_high_gain: FloatParam,

    // --- Reverb Ducking ---
    #[id = "reverb_ducking"]
    pub reverb_ducking: FloatParam,

    // --- Delay Ducking / Ping Pong ---
    #[id = "delay_ducking"]
    pub delay_ducking: FloatParam,

    #[id = "delay_ping_pong"]
    pub delay_ping_pong: BoolParam,

    #[id = "amp_normalize"]
    pub amp_normalize: BoolParam,

    #[id = "cab_normalize"]
    pub cab_normalize: BoolParam,

    #[id = "snapshot"]
    pub snapshot: IntParam,

    #[id = "macro_1"]
    pub macro_1: FloatParam,

    #[id = "macro_2"]
    pub macro_2: FloatParam,

    #[id = "macro_3"]
    pub macro_3: FloatParam,

    #[id = "macro_4"]
    pub macro_4: FloatParam,

    #[persist = "snapshots_json"]
    pub snapshots_json: Arc<Mutex<String>>,

    #[persist = "macro_mappings_json"]
    pub macro_mappings_json: Arc<Mutex<String>>,

    #[persist = "slots_json"]
    pub slots_json: Arc<Mutex<String>>,

    #[persist = "nam_model_name"]
    pub nam_model_name: Arc<Mutex<String>>,
    #[persist = "nam_model_path"]
    pub nam_model_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    #[persist = "cab_ir_name"]
    pub cab_ir_name: Arc<Mutex<String>>,
    #[persist = "cab_ir_path"]
    pub cab_ir_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    #[persist = "routing_order"]
    pub routing_order: Arc<Mutex<[usize; 5]>>,

    pub rt_snapshots: arc_swap::ArcSwap<registry::RtSnapshotInfo>,
    pub rt_mappings: arc_swap::ArcSwap<registry::RtMacroMappings>,
    pub captured_samples: Arc<Mutex<Vec<f32>>>,

    pub nam_cache: Arc<Mutex<std::collections::HashMap<std::path::PathBuf, Arc<nam_rs::NamModel>>>>,
    pub cab_cache: Arc<Mutex<std::collections::HashMap<std::path::PathBuf, (Vec<f32>, Vec<f32>)>>>,

    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
}

fn get_top_file_in_dir(dir_path: &str, ext: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(dir_path);
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut files: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .map_or(false, |e| e.eq_ignore_ascii_case(ext))
                })
                .collect();
            files.sort();
            if !files.is_empty() {
                return Some(files[0].clone());
            }
        }
    }
    None
}

impl SlotsFxParams {
    pub fn new() -> Self {
        let mut default_model_name = String::new();
        let mut default_model_path = None;
        if let Some(path) = get_top_file_in_dir(r"C:\LIBRARIES\NAM MODELS\DIEZEL VH4", "nam") {
            default_model_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            default_model_path = Some(path);
        }

        let mut default_cab_name = "Default (Passthrough)".to_string();
        let mut default_cab_path = None;
        if let Some(path) = get_top_file_in_dir(r"C:\LIBRARIES\IR", "wav") {
            default_cab_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            default_cab_path = Some(path);
        }

        let default_slots: Vec<serde_json::Value> = vec![];
        let default_slots_json = serde_json::to_string(&default_slots).unwrap_or_default();

        let default_snapshots: Vec<serde_json::Value> = (0..8).map(|_| {
            serde_json::json!({
                "slots": default_slots,
                "params": {}
            })
        }).collect();
        let default_snapshots_json = serde_json::to_string(&default_snapshots).unwrap_or_default();
        let default_mappings_json = "[]".to_string();

        Self {
            input_gain: FloatParam::new(
                "Input Gain",
                0.0,
                FloatRange::Linear { min: -24.0, max: 24.0 },
            ),
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear { min: -24.0, max: 24.0 },
            ),
            mix: FloatParam::new(
                "Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            pitch_gain: FloatParam::new(
                "Pitch Gain",
                0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 },
            ),
            nam_gain: FloatParam::new(
                "Nam Gain",
                0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 },
            ),
            cab_gain: FloatParam::new(
                "Cab Gain",
                0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 },
            ),
            delay_mix: FloatParam::new(
                "Delay Mix",
                0.3,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            delay_feedback: FloatParam::new(
                "Delay Feedback",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            reverb_mix: FloatParam::new(
                "Reverb Mix",
                0.3,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            reverb_space: FloatParam::new(
                "Reverb Space",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            pitch_bypass: BoolParam::new("Pitch Bypass", false),
            nam_bypass: BoolParam::new("Nam Bypass", false),
            cab_bypass: BoolParam::new("Cab Bypass", false),
            delay_bypass: BoolParam::new("Delay Bypass", false),
            reverb_bypass: BoolParam::new("Reverb Bypass", false),

            amp_bass: FloatParam::new("Bass", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            amp_middle: FloatParam::new("Middle", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            amp_high: FloatParam::new("High", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            amp_output: FloatParam::new("Output", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }),

            amp_bass_freq: FloatParam::new("Bass Freq", 150.0, FloatRange::Linear { min: 80.0, max: 400.0 }),
            amp_mid_freq: FloatParam::new("Mid Freq", 425.0, FloatRange::Linear { min: 200.0, max: 2000.0 }),
            amp_high_freq: FloatParam::new("High Freq", 1800.0, FloatRange::Linear { min: 1000.0, max: 8000.0 }),

            gate_bypass: BoolParam::new("Gate Bypass", false),
            gate_threshold: FloatParam::new("Gate Threshold", -40.0, FloatRange::Linear { min: -60.0, max: 0.0 }),
            gate_attack: FloatParam::new("Gate Attack", 5.0, FloatRange::Linear { min: 0.1, max: 50.0 }),
            gate_release: FloatParam::new("Gate Release", 100.0, FloatRange::Linear { min: 10.0, max: 500.0 }),

            cab_position: FloatParam::new("Position", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            cab_size: FloatParam::new("Size", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),

            pitch_semi: FloatParam::new("Semitones", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }),
            pitch_mix: FloatParam::new("Pitch Mix", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),

            delay_time: FloatParam::new("Time", 250.0, FloatRange::Linear { min: 50.0, max: 1000.0 }),

            reverb_shimmer: FloatParam::new("Shimmer", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),

            amp_normalize: BoolParam::new("Amp Normalize", true),
            cab_normalize: BoolParam::new("Cab Normalize", true),

            bitcrush_bits: FloatParam::new("Bits", 24.0, FloatRange::Linear { min: 1.0, max: 24.0 }),
            bitcrush_downsample: FloatParam::new("Downsample", 1.0, FloatRange::Linear { min: 1.0, max: 32.0 }),
            bitcrush_mix: FloatParam::new("Bitcrush Mix", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            bitcrush_mode: FloatParam::new("Bitcrush Mode", 0.0, FloatRange::Linear { min: 0.0, max: 2.0 }),

            overdrive_drive: FloatParam::new("Drive", 20.0, FloatRange::Linear { min: 1.0, max: 100.0 }),
            overdrive_tone: FloatParam::new("Tone", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            overdrive_level: FloatParam::new("Level", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            overdrive_algo: FloatParam::new("Overdrive Algo", 0.0, FloatRange::Linear { min: 0.0, max: 2.0 }),

            eq_low_freq: FloatParam::new("EQ Low Freq", 100.0, FloatRange::Linear { min: 20.0, max: 1000.0 }),
            eq_low_gain: FloatParam::new("EQ Low Gain", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }),
            eq_mid_freq: FloatParam::new("EQ Mid Freq", 1000.0, FloatRange::Linear { min: 200.0, max: 5000.0 }),
            eq_mid_gain: FloatParam::new("EQ Mid Gain", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }),
            eq_mid_q: FloatParam::new("EQ Mid Q", 1.0, FloatRange::Linear { min: 0.1, max: 10.0 }),
            eq_high_freq: FloatParam::new("EQ High Freq", 5000.0, FloatRange::Linear { min: 1000.0, max: 20000.0 }),
            eq_high_gain: FloatParam::new("EQ High Gain", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }),

            reverb_ducking: FloatParam::new("Reverb Ducking", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            delay_ducking: FloatParam::new("Delay Ducking", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            delay_ping_pong: BoolParam::new("Delay Ping-Pong", false),

            snapshot: IntParam::new("Snapshot", 0, IntRange::Linear { min: 0, max: 7 }),
            macro_1: FloatParam::new("Macro 1", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_2: FloatParam::new("Macro 2", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_3: FloatParam::new("Macro 3", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            macro_4: FloatParam::new("Macro 4", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            snapshots_json: Arc::new(Mutex::new(default_snapshots_json.clone())),
            macro_mappings_json: Arc::new(Mutex::new(default_mappings_json.clone())),

            slots_json: Arc::new(Mutex::new(default_slots_json)),
            nam_model_name: Arc::new(Mutex::new(default_model_name)),
            nam_model_path: Arc::new(Mutex::new(default_model_path)),
            cab_ir_name: Arc::new(Mutex::new(default_cab_name)),
            cab_ir_path: Arc::new(Mutex::new(default_cab_path)),
            routing_order: Arc::new(Mutex::new([1, 2, 0, 3, 4])),

            rt_snapshots: arc_swap::ArcSwap::new(Arc::new(registry::parse_snapshots(&default_snapshots_json))),
            rt_mappings: arc_swap::ArcSwap::new(Arc::new(registry::parse_macro_mappings(&default_mappings_json))),
            captured_samples: Arc::new(Mutex::new(Vec::new())),

            nam_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cab_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            editor_state: EguiState::from_size(900, 780),
        }
    }
}

impl Default for SlotsFx {
    fn default() -> Self {
        let (blocks_p, blocks_c) = RingBuffer::new(2);
        let (nam_p, nam_c) = RingBuffer::new(2);
        let (cab_p, cab_c) = RingBuffer::new(2);
        let (cab_norm_p, cab_norm_c) = RingBuffer::new(2);
        let (routing_p, routing_c) = RingBuffer::new(2);
        let params = Arc::new(SlotsFxParams::new());

        let mut processor = PluginProcessor::new();

        let default_slots_json = params.slots_json.lock().unwrap().clone();
        if let Ok(slots) = serde_json::from_str::<Vec<serde_json::Value>>(&default_slots_json) {
            let mut starting_blocks = Vec::new();
            for s in slots {
                let slot_id = s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let bypassed = s.get("bypassed").and_then(|v| v.as_bool()).unwrap_or(false);
                let slot_type = s.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let path = s.get("path").and_then(|v| v.as_str()).map(std::path::PathBuf::from);
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let pan = s.get("pan").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let lane = s.get("lane").and_then(|v| v.as_str()).unwrap_or("serial").to_string();
                match slot_type {
                    "amp" => {
                        let mut block = NamBlock::new();
                        if let Some(ref p) = path {
                            if p.exists() {
                                match nam_rs::NamModel::from_file(p) {
                                    Ok(model_file) => {
                                        let loudness = model_file.loudness();
                                        if let (Ok(ml), Ok(mr)) = (
                                            nam_rs::Model::from_nam(&model_file),
                                            nam_rs::Model::from_nam(&model_file),
                                        ) {
                                            block.set_models(Some(ml), Some(mr), loudness);
                                            params.nam_cache.lock().unwrap().insert(p.clone(), Arc::new(model_file));
                                        }
                                    }
                                    Err(err) => eprintln!("Failed to load NAM model {:?}: {}", p, err),
                                }
                            } else {
                                eprintln!("NAM model path does not exist: {:?}", p);
                            }
                        }
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Nam {
                                block,
                                model_path: path,
                                model_name: name,
                                eq: dsp::eq::ParametricEq::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "cab" => {
                        let mut convolver = CabConvolver::new();
                        if let Some(ref p) = path {
                            if p.exists() {
                                match hound::WavReader::open(p) {
                                    Ok(mut reader) => {
                                        let spec = reader.spec();
                                        let samples: Vec<f32> = match spec.sample_format {
                                            hound::SampleFormat::Float => {
                                                reader.samples::<f32>()
                                                    .filter_map(Result::ok)
                                                    .map(|s| if s.is_nan() || s.is_infinite() { 0.0 } else { s })
                                                    .collect()
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
                                            convolver.set_ir(ir_l.clone(), ir_r.clone(), true);
                                            params.cab_cache.lock().unwrap().insert(p.clone(), (ir_l, ir_r));
                                        }
                                    }
                                    Err(err) => eprintln!("Failed to load cab IR {:?}: {}", p, err),
                                }
                            } else {
                                eprintln!("Cab IR path does not exist: {:?}", p);
                            }
                        }
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Cab {
                                convolver,
                                ir_path: path,
                                ir_name: name,
                                normalize: true,
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "delay" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Delay {
                                delay: dsp::delay::Delay::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "shimmer" | "verb" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Reverb {
                                reverb: dsp::reverb::Reverb::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "gate" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Gate {
                                gate: dsp::gate::NoiseGate::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "error" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Bitcrusher {
                                crusher: dsp::bitcrusher::Bitcrusher::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "od" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Overdrive {
                                od: dsp::overdrive::Overdrive::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "eq" => {
                        starting_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Eq {
                                eq: dsp::eq::ParametricEq::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    _ => {}
                }
            }
            if !starting_blocks.is_empty() {
                processor.blocks = starting_blocks;
            }
        }

        let instance_id = registry::next_instance_id();
        let shared_data = Arc::new(registry::InstanceSharedData::new(instance_id));
        
        if let Ok(mut reg) = registry::get_registry().lock() {
            reg.instances.insert(instance_id, shared_data.clone());
        }

        Self {
            params: params.clone(),
            processor,
            dsp_metrics: Arc::new(DspMetrics::new()),
            
            blocks_receiver: blocks_c,
            blocks_sender: Arc::new(Mutex::new(Some(blocks_p))),

            nam_models_receiver: nam_c,
            nam_sender: Arc::new(Mutex::new(Some(nam_p))),

            cab_irs_receiver: cab_c,
            cab_sender: Arc::new(Mutex::new(Some(cab_p))),

            cab_normalize_receiver: cab_norm_c,
            cab_normalize_sender: Arc::new(Mutex::new(Some(cab_norm_p))),

            routing_receiver: routing_c,
            routing_sender: Arc::new(Mutex::new(Some(routing_p))),

            nam_model_name: params.nam_model_name.clone(),
            nam_model_path: params.nam_model_path.clone(),
            cab_ir_name: params.cab_ir_name.clone(),
            cab_ir_path: params.cab_ir_path.clone(),
            routing_order: params.routing_order.clone(),

            dry_left_buffer: Vec::new(),
            dry_right_buffer: Vec::new(),

            instance_id,
            shared_data,
            sender_ring: None,
            receiver_ring: None,
            sweep_sample_counter: 0,
            capture_active: false,
            capture_latency_gate: false,
            capture_buffer: Vec::new(),
            capture_write_ptr: 0,
        }
    }
}

impl Drop for SlotsFx {
    fn drop(&mut self) {
        if let Ok(mut reg) = registry::get_registry().lock() {
            reg.instances.remove(&self.instance_id);
        }
    }
}

impl Plugin for SlotsFx {
    const NAME: &'static str = "SlotsFX";
    const VENDOR: &'static str = "SlopDSP";
    const URL: &'static str = "https://github.com/your-username/slotsfx";
    const EMAIL: &'static str = "info@slopdsp.com";
    const VERSION: &'static str = "0.1.0";

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: std::num::NonZeroU32::new(2),
            main_output_channels: std::num::NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        }
    ];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let data = SlotsEditorData {
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
        };
        ui_webview::create_webview_editor(data, self.params.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layouts: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.dry_left_buffer.resize(buffer_config.max_buffer_size as usize, 0.0);
        self.dry_right_buffer.resize(buffer_config.max_buffer_size as usize, 0.0);
        self.dsp_metrics.buffer_size.store(buffer_config.max_buffer_size as u32, std::sync::atomic::Ordering::Relaxed);
        self.dsp_metrics.sample_rate.store(buffer_config.sample_rate as u32, std::sync::atomic::Ordering::Relaxed);
        self.processor.sample_rate = buffer_config.sample_rate as f32;

        let slots_json = self.params.slots_json.lock().unwrap().clone();
        if let Ok(slots) = serde_json::from_str::<Vec<serde_json::Value>>(&slots_json) {
            let mut restored_blocks = Vec::new();
            
            let mut nam_cache = self.params.nam_cache.lock().unwrap();
            let mut cab_cache = self.params.cab_cache.lock().unwrap();

             for s in slots {
                let slot_id = s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let bypassed = s.get("bypassed").and_then(|v| v.as_bool()).unwrap_or(false);
                let slot_type = s.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let path = s.get("path").and_then(|v| v.as_str()).map(std::path::PathBuf::from);
                let pan = s.get("pan").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let lane = s.get("lane").and_then(|v| v.as_str()).unwrap_or("serial").to_string();
                // Extract cab_normalize from slot params if present
                let cab_normalize = s.get("params")
                    .and_then(|v| v.get("cab_normalize"))
                    .and_then(|v| v.as_f64())
                    .map(|f| f > 0.5)
                    .unwrap_or(true);

                match slot_type {
                    "pitch" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Pitch,
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "amp" => {
                        let mut block = NamBlock::new();
                        if let Some(ref p) = path {
                            if let Some(model_file) = nam_cache.get(p) {
                                let loudness = model_file.loudness();
                                if let (Ok(ml), Ok(mr)) = (
                                    nam_rs::Model::from_nam(model_file),
                                    nam_rs::Model::from_nam(model_file),
                                ) {
                                    block.set_models(Some(ml), Some(mr), loudness);
                                }
                            } else if p.exists() {
                                match nam_rs::NamModel::from_file(p) {
                                    Ok(model_file) => {
                                        let loudness = model_file.loudness();
                                        if let (Ok(ml), Ok(mr)) = (
                                            nam_rs::Model::from_nam(&model_file),
                                            nam_rs::Model::from_nam(&model_file),
                                        ) {
                                            block.set_models(Some(ml), Some(mr), loudness);
                                            nam_cache.insert(p.clone(), Arc::new(model_file));
                                        }
                                    }
                                    Err(err) => eprintln!("Failed to load NAM model {:?}: {}", p, err),
                                }
                            } else {
                                eprintln!("NAM model path does not exist: {:?}", p);
                            }
                        }
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Nam {
                                block,
                                model_path: path.clone(),
                                model_name: name.clone(),
                                eq: dsp::eq::ParametricEq::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "cab" => {
                        let mut convolver = CabConvolver::new();
                        if let Some(ref p) = path {
                            if let Some((ir_l, ir_r)) = cab_cache.get(p) {
                                convolver.set_ir(ir_l.clone(), ir_r.clone(), true);
                            } else if p.exists() {
                                match hound::WavReader::open(p) {
                                    Ok(mut reader) => {
                                        let spec = reader.spec();
                                        let samples: Vec<f32> = match spec.sample_format {
                                            hound::SampleFormat::Float => {
                                                reader.samples::<f32>()
                                                    .filter_map(Result::ok)
                                                    .map(|s| if s.is_nan() || s.is_infinite() { 0.0 } else { s })
                                                    .collect()
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
                                            convolver.set_ir(ir_l.clone(), ir_r.clone(), true);
                                            cab_cache.insert(p.clone(), (ir_l, ir_r));
                                        }
                                    }
                                    Err(err) => eprintln!("Failed to load cab IR {:?}: {}", p, err),
                                }
                            } else {
                                eprintln!("Cab IR path does not exist: {:?}", p);
                            }
                        }
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Cab {
                                convolver,
                                ir_path: path.clone(),
                                ir_name: name.clone(),
                                normalize: cab_normalize,
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "delay" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Delay {
                                delay: dsp::delay::Delay::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "shimmer" | "verb" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Reverb {
                                reverb: dsp::reverb::Reverb::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "gate" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Gate {
                                gate: dsp::gate::NoiseGate::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "error" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Bitcrusher {
                                crusher: dsp::bitcrusher::Bitcrusher::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "od" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Overdrive {
                                od: dsp::overdrive::Overdrive::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    "eq" => {
                        restored_blocks.push(EffectBlock {
                            id: slot_id,
                            slot_type: slot_type.to_string(),
                            bypassed,
                            pan,
                            lane,
                            effect: crate::dsp::EffectType::Eq {
                                eq: dsp::eq::ParametricEq::new(),
                            },
                            params: HashMap::new(),
                            tail_out: false,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                    _ => {}
                }
            }
            if !restored_blocks.is_empty() {
                self.processor.blocks = restored_blocks;
            }
        }

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        use std::sync::atomic::Ordering;
        let start_time = std::time::Instant::now();

        while let Ok(new_configs) = self.blocks_receiver.pop() {
            self.processor.update_from_configs(new_configs, &self.params);
        }

        // Drain any freshly loaded NAM models from the editor (load_nam IPC).
        while let Ok((ml, mr, loudness)) = self.nam_models_receiver.pop() {
            if let Some(block) = self.processor.blocks.iter_mut().find_map(|b| match &mut b.effect {
                crate::dsp::EffectType::Nam { block, .. } => Some(block),
                _ => None,
            }) {
                block.set_models(Some(ml), Some(mr), loudness);
            }
        }

        // Drain any freshly loaded cab IRs from the editor (load_cab IPC).
        while let Ok((ir_l, ir_r)) = self.cab_irs_receiver.pop() {
            for block in &mut self.processor.blocks {
                if let crate::dsp::EffectType::Cab { convolver, normalize, .. } = &mut block.effect {
                    convolver.set_ir(ir_l.clone(), ir_r.clone(), *normalize);
                }
            }
        }

        // Also drain normalize toggle messages sent via the dedicated channel
        while let Ok((normalize, ref ir_l, ref ir_r)) = self.cab_normalize_receiver.pop() {
            for block in &mut self.processor.blocks {
                if let crate::dsp::EffectType::Cab { convolver, .. } = &mut block.effect {
                    if let (Some(l), Some(r)) = (ir_l.as_ref(), ir_r.as_ref()) {
                        // Reload IR with new normalize state
                        convolver.set_ir(l.clone(), r.clone(), normalize);
                    } else {
                        // Just toggle normalization without reloading IR
                        convolver.set_normalize(normalize);
                    }
                }
            }
        }

        // Drain any routing-order changes from the editor (rack reorder).
        // Currently a no-op on the audio side: the editor's own routing_order state
        // is the source of truth; the audio thread processes blocks in declaration order.
        while let Ok(_new_order) = self.routing_receiver.pop() {}

        let input_gain_db = self.params.input_gain.value();
        let input_gain_coeff = 10.0_f32.powf(input_gain_db / 20.0);
        let mix_val = self.params.mix.value();

        let channel_slices = buffer.as_slice();
        if channel_slices.len() >= 2 {
            let (left_slice, right_slice_rest) = channel_slices.split_at_mut(1);
            let left_channel = &mut left_slice[0];
            let right_channel = &mut right_slice_rest[0];
            let buffer_len = left_channel.len();
            let sample_rate = self.processor.sample_rate;

            self.dsp_metrics.buffer_size.store(buffer_len as u32, Ordering::Relaxed);
            self.dsp_metrics.sample_rate.store(sample_rate as u32, Ordering::Relaxed);

            if self.sender_ring.is_none() {
                if let Ok(mut lock) = self.shared_data.dry_buffer_tx.lock() {
                    self.sender_ring = lock.take();
                }
            }
            if self.receiver_ring.is_none() {
                if let Ok(mut lock) = self.shared_data.dry_buffer_rx.lock() {
                    self.receiver_ring = lock.take();
                }
            }

            let is_sender = self.shared_data.is_sender.load(Ordering::Relaxed);
            let is_receiver = self.shared_data.is_receiver.load(Ordering::Relaxed);
            let ab_mode = self.shared_data.ab_mode.load(Ordering::Relaxed);

            let mut using_streamed_dry = false;
            if is_receiver && ab_mode == 2 {
                if let Some(ref mut cons) = self.receiver_ring {
                    if cons.slots() >= buffer_len {
                        for j in 0..buffer_len {
                            let s = cons.pop().unwrap_or(0.0);
                            left_channel[j] = s;
                            right_channel[j] = s;
                        }
                        using_streamed_dry = true;
                    }
                }
            }

            let mut sweep_active = self.shared_data.sweep_active.load(Ordering::Relaxed);
            let sweep_trigger = self.shared_data.sweep_trigger.load(Ordering::Relaxed);
            if is_sender && (sweep_trigger || sweep_active) {
                if sweep_trigger {
                    self.shared_data.sweep_trigger.store(false, Ordering::Relaxed);
                    self.shared_data.sweep_active.store(true, Ordering::Relaxed);
                    self.sweep_sample_counter = 0;
                    sweep_active = true;
                }

                let duration_secs = 5.0f32;
                let total_samples = (duration_secs * sample_rate) as u32;
                let f1 = 20.0f32;
                let f2 = 20000.0f32;
                let ln_f2_f1 = (f2 / f1).ln();
                let factor = 2.0f32 * std::f32::consts::PI * f1 * duration_secs / ln_f2_f1;

                for j in 0..buffer_len {
                    if self.sweep_sample_counter < total_samples {
                        let t = self.sweep_sample_counter as f32 / sample_rate;
                        let exponent = (t / duration_secs) * ln_f2_f1;
                        let theta = factor * (exponent.exp() - 1.0f32);
                        let val = theta.sin();

                        let fade_samples = (0.01 * sample_rate) as u32;
                        let mut fade = 1.0f32;
                        if self.sweep_sample_counter < fade_samples {
                            fade = self.sweep_sample_counter as f32 / fade_samples as f32;
                        } else if self.sweep_sample_counter > total_samples - fade_samples {
                            fade = (total_samples - self.sweep_sample_counter) as f32 / fade_samples as f32;
                        }

                        let sweep_sample = val * fade;
                        left_channel[j] = sweep_sample;
                        right_channel[j] = sweep_sample;

                        self.sweep_sample_counter += 1;
                    } else {
                        self.shared_data.sweep_active.store(false, Ordering::Relaxed);
                        left_channel[j] = 0.0;
                        right_channel[j] = 0.0;
                    }
                }
            } else {
                if !using_streamed_dry {
                    for sample in left_channel.iter_mut() {
                        *sample *= input_gain_coeff;
                    }
                    for sample in right_channel.iter_mut() {
                        *sample *= input_gain_coeff;
                    }
                }
            }

            if is_receiver {
                let rec_active = self.capture_active;
                if !rec_active {
                    if self.shared_data.sweep_trigger.load(Ordering::Relaxed) {
                        self.shared_data.sweep_trigger.store(false, Ordering::Relaxed);
                        self.capture_active = true;
                        self.capture_latency_gate = false;
                        self.capture_write_ptr = 0;
                        let len = (5.2f32 * sample_rate) as usize;
                        self.capture_buffer = vec![0.0f32; len];
                    }
                }

                if self.capture_active {
                    let start_idx = 0;
                    let to_copy = &left_channel[start_idx..];
                    let write_len = to_copy.len().min(self.capture_buffer.len() - self.capture_write_ptr);
                    self.capture_buffer[self.capture_write_ptr..self.capture_write_ptr + write_len]
                        .copy_from_slice(&to_copy[..write_len]);
                    self.capture_write_ptr += write_len;

                    if self.capture_write_ptr >= self.capture_buffer.len() {
                        self.capture_active = false;
                        if let Ok(mut guard) = self.params.captured_samples.lock() {
                            *guard = self.capture_buffer.clone();
                        }
                        self.shared_data.sweep_progress.store(999999, Ordering::Relaxed);
                    } else {
                        self.shared_data.sweep_progress.store(self.capture_write_ptr as u32, Ordering::Relaxed);
                    }
                }
            }

            if is_sender && ab_mode == 2 {
                if let Some(ref mut prod) = self.sender_ring {
                    if prod.slots() >= buffer_len {
                        for &s in left_channel.iter() {
                            let _ = prod.push(s);
                        }
                    }
                }
                for sample in left_channel.iter_mut() { *sample = 0.0; }
                for sample in right_channel.iter_mut() { *sample = 0.0; }
            }

            let mut input_peak = 0.0f32;
            for &sample in left_channel.iter() {
                input_peak = input_peak.max(sample.abs());
            }
            for &sample in right_channel.iter() {
                input_peak = input_peak.max(sample.abs());
            }
            self.dsp_metrics.input_peak_level_bits.store(input_peak.to_bits(), Ordering::Relaxed);

            if buffer_len <= self.dry_left_buffer.len() {
                self.dry_left_buffer[..buffer_len].copy_from_slice(left_channel);
                self.dry_right_buffer[..buffer_len].copy_from_slice(right_channel);
            }

            let skip_processing = (is_sender && sweep_active) || (is_receiver && ab_mode == 1);
            if !skip_processing {
                self.processor.process(
                    left_channel,
                    right_channel,
                    &self.params,
                    &self.dsp_metrics.nam_time_ns,
                    &self.dsp_metrics.cab_time_ns,
                    &self.dsp_metrics.slot_peaks,
                );
            }

            // Blend Dry/Wet mix
            if buffer_len <= self.dry_left_buffer.len() {
                for i in 0..buffer_len {
                    let dry_l = self.dry_left_buffer[i];
                    let dry_r = self.dry_right_buffer[i];
                    left_channel[i] = left_channel[i] * mix_val + dry_l * (1.0 - mix_val);
                    right_channel[i] = right_channel[i] * mix_val + dry_r * (1.0 - mix_val);
                }
            }

            // Output Gain
            let output_gain_db = self.params.output_gain.value();
            let output_gain_coeff = 10.0_f32.powf(output_gain_db / 20.0);
            for sample in left_channel.iter_mut() {
                *sample *= output_gain_coeff;
            }
            for sample in right_channel.iter_mut() {
                *sample *= output_gain_coeff;
            }

            // Peak level for display
            let mut peak = 0.0f32;
            for &sample in left_channel.iter() {
                peak = peak.max(sample.abs());
            }
            for &sample in right_channel.iter() {
                peak = peak.max(sample.abs());
            }
            self.dsp_metrics.peak_level_bits.store(peak.to_bits(), Ordering::Relaxed);

            let elapsed = start_time.elapsed();
            self.dsp_metrics.process_time_ns.store(elapsed.as_nanos() as u32, Ordering::Relaxed);
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for SlotsFx {
    const VST3_CLASS_ID: [u8; 16] = [
        b'S', b'l', b'o', b't', b's', b'F', b'X', b'S',
        b'l', b'o', b'p', b'D', b'S', b'P', 0, 0,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Distortion,
    ];
}

impl ClapPlugin for SlotsFx {
    const CLAP_ID: &'static str = "com.slopdsp.slotsfx";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("SlotsFX amp sim and effects rack");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
    ];
}

nih_export_vst3!(SlotsFx);
nih_export_clap!(SlotsFx);

