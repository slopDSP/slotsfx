pub mod nam;
pub mod cab;
pub mod delay;
pub mod reverb;
pub mod gate;
pub mod bitcrusher;
pub mod overdrive;
pub mod eq;
pub mod deconvolve;
pub mod pitch_detector;
pub mod pitch_shifter;
pub mod tuner;
pub mod autotune;

use std::collections::HashMap;
use std::sync::Arc;
use crate::SlotsFxParams;

pub struct SlotConfig {
    pub id: String,
    pub slot_type: String,
    pub name: String,
    pub path: Option<std::path::PathBuf>,
    pub bypassed: bool,
    pub pan: f32,
    pub lane: String,
    pub params: HashMap<String, f32>,
}

pub struct EffectBlock {
    pub id: String,
    pub slot_type: String,
    pub bypassed: bool,
    pub pan: f32,
    pub lane: String,
    pub effect: EffectType,
    pub params: HashMap<String, f32>,
    /// When true, bypass feeds silence but keeps DSP running so the tail rings out.
    pub tail_out: bool,
    /// True while the tail is actively fading out after bypass.
    pub fading_out: bool,
    /// Current gain multiplier for fade-out ramp (1.0 = full, 0.0 = silent).
    pub fade_gain: f32,
}

pub enum EffectType {
    Pitch {
        shifter: pitch_shifter::PitchShifter,
    },
    Nam {
        block: nam::NamBlock,
        model_path: Option<std::path::PathBuf>,
        model_name: String,
        eq: eq::ParametricEq,
    },
    Cab {
        convolver: cab::CabConvolver,
        ir_path: Option<std::path::PathBuf>,
        ir_name: String,
        normalize: bool,
    },
    Delay {
        delay: delay::Delay,
    },
    Reverb {
        reverb: reverb::Reverb,
    },
    Gate {
        gate: gate::NoiseGate,
    },
    Bitcrusher {
        crusher: bitcrusher::Bitcrusher,
    },
    Overdrive {
        od: overdrive::Overdrive,
    },
    Eq {
        eq: eq::ParametricEq,
    },
}

/// Returns true for slot types whose tails should ring out naturally.
pub fn is_space_effect(slot_type: &str) -> bool {
    matches!(slot_type, "delay" | "verb" | "shimmer")
}

pub struct PluginProcessor {
    pub blocks: Vec<EffectBlock>,
    /// Blocks that have been removed from the config but are still fading out.
    pub fading_out_blocks: Vec<EffectBlock>,
    pub sample_rate: f32,
    /// Samples remaining in fade-in ramp after config update (masks clicks).
    fade_ramp_remaining: u32,
    pub tuner: tuner::Tuner,
    pub autotune: autotune::AutoTune,
}

impl PluginProcessor {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            fading_out_blocks: Vec::new(),
            sample_rate: 48000.0,
            fade_ramp_remaining: 0,
            tuner: tuner::Tuner::new(48000.0),
            autotune: autotune::AutoTune::new(48000.0, autotune::AutoTuneConfig::default()),
        }
    }

    /// Apply a fade-in ramp to mask clicks after a config update.
    fn apply_fade_ramp(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.fade_ramp_remaining == 0 {
            return;
        }
        let ramp_len = self.fade_ramp_remaining.min(left.len() as u32) as usize;
        for i in 0..ramp_len {
            let t = i as f32 / ramp_len as f32;
            // Cosine fade: 0 → 1 (smooth start, no discontinuity)
            let fade = 1.0 - f32::cos(std::f32::consts::PI * t * 0.5);
            left[i] *= fade;
            right[i] *= fade;
        }
        self.fade_ramp_remaining -= ramp_len as u32;
    }

    pub fn update_from_configs(&mut self, configs: Vec<SlotConfig>, params: &SlotsFxParams) {
        // Build lookup of existing blocks by id so we can reuse their DSP state.
        let mut existing: std::collections::HashMap<_, _> = self
            .blocks
            .drain(..)
            .map(|b| (b.id.clone(), b))
            .collect();

        let mut new_blocks: Vec<EffectBlock> = Vec::new();
        let mut nam_cache = params.nam_cache.lock().unwrap();
        let mut cab_cache = params.cab_cache.lock().unwrap();

        for config in configs {
            let slot_type = config.slot_type.as_str();
            let tail_out = config
                .params
                .get("tail_out")
                .copied()
                .unwrap_or(0.0)
                > 0.5;

            match slot_type {
                "pitch" => {
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Pitch {
                            shifter: pitch_shifter::PitchShifter::new(self.sample_rate, pitch_shifter::ShiftMode::PsoLa),
                        },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "amp" => {
                    // Reuse existing NAM block when model path hasn't changed
                    let reused = if let Some(mut existing_block) = existing.remove(&config.id) {
                        let should_reuse = match &existing_block.effect {
                            EffectType::Nam { model_path, .. } => match (model_path, &config.path) {
                                (Some(a), Some(b)) => a == b,
                                (None, None) => true,
                                _ => false,
                            },
                            _ => false,
                        };
                        if should_reuse {
                            if let EffectType::Nam { model_path, model_name, .. } = &mut existing_block.effect {
                                *model_path = config.path.clone();
                                *model_name = config.name.clone();
                            }
                            existing_block.bypassed = config.bypassed;
                            existing_block.pan = config.pan;
                            existing_block.lane = config.lane.clone();
                            existing_block.params = config.params.clone();
                            existing_block.tail_out = tail_out;
                            existing_block.fading_out = false;
                            existing_block.fade_gain = 1.0;
                            new_blocks.push(existing_block);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !reused {
                        let mut block = nam::NamBlock::new();
                        if let Some(ref p) = config.path {
                            if let Some(model_file) = nam_cache.get(p) {
                                let loudness = model_file.loudness();
                                if let (Ok(ml), Ok(mr)) = (
                                    nam_rs::Model::from_nam(model_file),
                                    nam_rs::Model::from_nam(model_file),
                                ) {
                                    block.set_models(Some(ml), Some(mr), loudness);
                                }
                            } else if p.exists() {
                                if let Ok(model_file) = nam_rs::NamModel::from_file(p) {
                                    let loudness = model_file.loudness();
                                    if let (Ok(ml), Ok(mr)) = (
                                        nam_rs::Model::from_nam(&model_file),
                                        nam_rs::Model::from_nam(&model_file),
                                    ) {
                                        block.set_models(Some(ml), Some(mr), loudness);
                                        nam_cache.insert(p.clone(), Arc::new(model_file));
                                    }
                                }
                            }
                        }
                        new_blocks.push(EffectBlock {
                            id: config.id,
                            slot_type: slot_type.to_string(),
                            bypassed: config.bypassed,
                            pan: config.pan,
                            lane: config.lane,
                            effect: EffectType::Nam {
                                block,
                                model_path: config.path,
                                model_name: config.name,
                                eq: eq::ParametricEq::new(),
                            },
                            params: config.params,
                            tail_out,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                }
                "cab" => {
                    let do_normalize = config.params.get("cab_normalize")
                        .copied()
                        .map(|v| v > 0.5)
                        .unwrap_or(true);

                    // Reuse existing cab block when IR path hasn't changed (preserves buffer, avoids clicks)
                    let reused = if let Some(mut existing_block) = existing.remove(&config.id) {
                        let should_reuse = match &existing_block.effect {
                            EffectType::Cab { ir_path, .. } => match (ir_path, &config.path) {
                                (Some(a), Some(b)) => a == b,
                                (None, None) => true,
                                _ => false,
                            },
                            _ => false,
                        };
                        if should_reuse {
                            if let EffectType::Cab { convolver, normalize, ir_path, ir_name } = &mut existing_block.effect {
                                if *normalize != do_normalize {
                                    convolver.set_normalize(do_normalize);
                                    *normalize = do_normalize;
                                }
                                *ir_path = config.path.clone();
                                *ir_name = config.name.clone();
                            }
                            existing_block.bypassed = config.bypassed;
                            existing_block.pan = config.pan;
                            existing_block.lane = config.lane.clone();
                            existing_block.params = config.params.clone();
                            existing_block.tail_out = tail_out;
                            existing_block.fading_out = false;
                            existing_block.fade_gain = 1.0;
                            new_blocks.push(existing_block);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !reused {
                        let mut convolver = cab::CabConvolver::new();
                        if let Some(ref p) = config.path {
                            if let Some((ir_l, ir_r)) = cab_cache.get(p) {
                                convolver.set_ir(ir_l.clone(), ir_r.clone(), do_normalize);
                            } else if p.exists() {
                                if let Ok(mut reader) = hound::WavReader::open(p) {
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
                                            (samples.iter().step_by(2).copied().collect(),
                                             samples.iter().skip(1).step_by(2).copied().collect())
                                        } else {
                                            (samples.clone(), samples)
                                        };
                                        convolver.set_ir(ir_l.clone(), ir_r.clone(), do_normalize);
                                        cab_cache.insert(p.clone(), (ir_l, ir_r));
                                    }
                                }
                            }
                        }
                        new_blocks.push(EffectBlock {
                            id: config.id,
                            slot_type: slot_type.to_string(),
                            bypassed: config.bypassed,
                            pan: config.pan,
                            lane: config.lane,
                            effect: EffectType::Cab {
                                convolver,
                                ir_path: config.path,
                                ir_name: config.name,
                                normalize: do_normalize,
                            },
                            params: config.params,
                            tail_out,
                            fading_out: false,
                            fade_gain: 1.0,
                        });
                    }
                }
                "delay" => {
                    // Reuse existing delay block to preserve its buffer state.
                    let delay = if let Some(EffectBlock {
                        effect: EffectType::Delay { delay: d },
                        ..
                    }) = existing.remove(&config.id)
                    {
                        d
                    } else {
                        delay::Delay::new()
                    };
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Delay { delay },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "shimmer" | "verb" => {
                    // Reuse existing reverb block to preserve its buffer state.
                    let reverb = if let Some(EffectBlock {
                        effect: EffectType::Reverb { reverb: r },
                        ..
                    }) = existing.remove(&config.id)
                    {
                        r
                    } else {
                        reverb::Reverb::new()
                    };
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Reverb { reverb },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "gate" => {
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Gate {
                            gate: gate::NoiseGate::new(),
                        },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "error" => {
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Bitcrusher {
                            crusher: bitcrusher::Bitcrusher::new(),
                        },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "od" => {
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Overdrive {
                            od: overdrive::Overdrive::new(),
                        },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                "eq" => {
                    new_blocks.push(EffectBlock {
                        id: config.id,
                        slot_type: slot_type.to_string(),
                        bypassed: config.bypassed,
                        pan: config.pan,
                        lane: config.lane,
                        effect: EffectType::Eq {
                            eq: eq::ParametricEq::new(),
                        },
                        params: config.params,
                        tail_out,
                        fading_out: false,
                        fade_gain: 1.0,
                    });
                }
                _ => {}
            }
        }

        // Move removed space-effect blocks to fading_out_blocks so tails can ring out.
        for (_, mut block) in existing {
            if is_space_effect(&block.slot_type) && block.tail_out {
                block.fading_out = true;
                block.bypassed = true;
                self.fading_out_blocks.push(block);
            }
            // Non-space or no-tail-out blocks are dropped.
        }

        self.blocks = new_blocks;

        // Fade-in ramp to mask clicks from block recreation / param changes
        self.fade_ramp_remaining = (self.sample_rate * 0.005) as u32; // 5ms
    }

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        params: &SlotsFxParams,
        nam_time_ns: &std::sync::atomic::AtomicU32,
        cab_time_ns: &std::sync::atomic::AtomicU32,
        _slot_peaks: &[std::sync::atomic::AtomicU32; 16],
        tuner_state: &std::sync::atomic::AtomicU64,
    ) {
        let mut to_drop = Vec::new();
        let fade_rate = 1.0 / (self.sample_rate * 0.5);

        // --- Run tuner on input (non-destructive) ---
        self.tuner.process(left);

        // --- Run auto-tune on input (modifies audio if enabled) ---
        {
            let autotune_cfg = autotune::AutoTuneConfig {
                enabled: params.auto_tune_toggle.value(),
                root_key: params.auto_tune_key.value() as u8,
                scale: match params.auto_tune_scale.value() as u8 {
                    0 => pitch_detector::ScaleType::Chromatic,
                    1 => pitch_detector::ScaleType::Major,
                    2 => pitch_detector::ScaleType::Minor,
                    _ => pitch_detector::ScaleType::Chromatic,
                },
                mode: if params.auto_tune_mode.value() {
                    pitch_shifter::ShiftMode::PhaseVocoder
                } else {
                    pitch_shifter::ShiftMode::PsoLa
                },
                retune_speed: params.auto_tune_speed.value(),
                correction_amount: params.auto_tune_amount.value(),
            };
            self.autotune.update_config(autotune_cfg);
            if autotune_cfg.enabled {
                let mut tun_l = vec![0.0f32; left.len()];
                let mut tun_r = vec![0.0f32; right.len()];
                self.autotune.process(left, &mut tun_l, 0);
                self.autotune.process(right, &mut tun_r, 1);
                left.copy_from_slice(&tun_l);
                right.copy_from_slice(&tun_r);
            }
        }

        // --- Publish tuner state to UI ---
        {
            let tuner_state_val = self.tuner.state();
            let packed: u64 = if tuner_state_val.active {
                let cents_scaled = (tuner_state_val.cents * 100.0).round() as i16;
                (cents_scaled as u64 & 0xFFFF) << 32
                    | ((tuner_state_val.note as u64) & 0xFF) << 24
                    | ((tuner_state_val.octave as u8 as u64) & 0xFF) << 16
                    | (1u64 << 8)
            } else {
                0u64
            };
            tuner_state.store(packed, std::sync::atomic::Ordering::Relaxed);
        }

        // --- Process fading_out_blocks first ---
        for (i, block) in self.fading_out_blocks.iter_mut().enumerate() {
            let slot_type = &block.slot_type;
            if matches!(slot_type.as_str(), "delay" | "verb" | "shimmer") {
                let mut silent_l = vec![0.0f32; left.len()];
                let mut silent_r = vec![0.0f32; right.len()];
                match &mut block.effect {
                    EffectType::Delay { delay } => {
                        let feedback = params.delay_feedback.value();
                        let mix = params.delay_mix.value();
                        let time = params.delay_time.value();
                        let ducking = params.delay_ducking.value();
                        let ping_pong = params.delay_ping_pong.value();
                        delay.process(&mut silent_l, &mut silent_r, self.sample_rate, time, feedback, mix, ducking, ping_pong);
                    }
                    EffectType::Reverb { reverb } => {
                        let mix = params.reverb_mix.value();
                        let space = params.reverb_space.value();
                        let shimmer = params.reverb_shimmer.value();
                        let ducking = params.reverb_ducking.value();
                        reverb.process(&mut silent_l, &mut silent_r, self.sample_rate, mix, space, shimmer, ducking);
                    }
                    _ => {}
                }
                for i in 0..left.len() {
                    left[i] += silent_l[i] * block.fade_gain;
                    right[i] += silent_r[i] * block.fade_gain;
                }
                block.fade_gain -= fade_rate * left.len() as f32;
                if block.fade_gain <= 0.0 {
                    to_drop.push(i);
                }
            }
        }
        for i in to_drop.into_iter().rev() {
            self.fading_out_blocks.remove(i);
        }

        // --- Process active blocks ---
        for block in &mut self.blocks {
            if block.bypassed {
                if block.tail_out && is_space_effect(&block.slot_type) {
                    block.fading_out = true;
                    block.fade_gain = 1.0;
                } else {
                    continue;
                }
            }

            let pitch_semi = params.pitch_semi.value();
            let pitch_mix = params.pitch_mix.value();

            if block.fading_out {
                let mut silent_l = vec![0.0f32; left.len()];
                let mut silent_r = vec![0.0f32; right.len()];
                match &mut block.effect {
                    EffectType::Delay { delay } => {
                        let feedback = params.delay_feedback.value();
                        let mix = params.delay_mix.value();
                        let time = params.delay_time.value();
                        let ducking = params.delay_ducking.value();
                        let ping_pong = params.delay_ping_pong.value();
                        delay.process(&mut silent_l, &mut silent_r, self.sample_rate, time, feedback, mix, ducking, ping_pong);
                    }
                    EffectType::Reverb { reverb } => {
                        let mix = params.reverb_mix.value();
                        let space = params.reverb_space.value();
                        let shimmer = params.reverb_shimmer.value();
                        let ducking = params.reverb_ducking.value();
                        reverb.process(&mut silent_l, &mut silent_r, self.sample_rate, mix, space, shimmer, ducking);
                    }
                    _ => {}
                }
                for i in 0..left.len() {
                    left[i] += silent_l[i] * block.fade_gain;
                    right[i] += silent_r[i] * block.fade_gain;
                }
                block.fade_gain -= fade_rate * left.len() as f32;
                if block.fade_gain <= 0.0 {
                    block.bypassed = true;
                    block.fading_out = false;
                }
                continue;
            }

            let pitch_period = if self.tuner.state().active {
                self.sample_rate / self.tuner.state().frequency
            } else {
                0.0
            };

            match &mut block.effect {
                EffectType::Pitch { shifter } => {
                    if pitch_mix <= 0.0 {
                        continue;
                    }
                    let mut wet_l = vec![0.0f32; left.len()];
                    let mut wet_r = vec![0.0f32; right.len()];
                    shifter.process(left, &mut wet_l, pitch_period, pitch_semi);
                    shifter.process(right, &mut wet_r, pitch_period, pitch_semi);
                    for i in 0..left.len() {
                        left[i] = left[i] * (1.0 - pitch_mix) + wet_l[i] * pitch_mix;
                        right[i] = right[i] * (1.0 - pitch_mix) + wet_r[i] * pitch_mix;
                    }
                }
                EffectType::Nam { block: nam_block, eq, .. } => {
                    let start = std::time::Instant::now();
                    nam_block.process(left, right, params.nam_gain.value());

                    // Post-model tone stack
                    let bass_gain = (params.amp_bass.value() - 0.5) * 24.0;
                    let mid_gain = (params.amp_middle.value() - 0.5) * 24.0;
                    let high_gain = (params.amp_high.value() - 0.5) * 24.0;
                    eq.process(left, right, self.sample_rate,
                        params.amp_bass_freq.value(), bass_gain,
                        params.amp_mid_freq.value(), mid_gain,
                        1.0,
                        params.amp_high_freq.value(), high_gain);

                    // Output makeup gain
                    let out_db = params.amp_output.value();
                    if out_db != 0.0 {
                        let g = 10.0_f32.powf(out_db / 20.0);
                        for s in left.iter_mut() { *s *= g; }
                        for s in right.iter_mut() { *s *= g; }
                    }

                    nam_time_ns.store(start.elapsed().as_nanos() as u32, std::sync::atomic::Ordering::Relaxed);
                }
                EffectType::Cab { .. } => {
                    // Check global cab_bypass — when on, signal passes through clean
                    if !params.cab_bypass.value() {
                        let start = std::time::Instant::now();
                        let position = params.cab_position.value();
                        let size = params.cab_size.value();
                        let convolver = match &mut block.effect {
                            EffectType::Cab { convolver, .. } => convolver,
                            _ => unreachable!(),
                        };
                        // Pass raw dB value — cab.rs does the dB→linear conversion
                        convolver.process(left, right, params.cab_gain.value(), position, size, self.sample_rate);
                        cab_time_ns.store(start.elapsed().as_nanos() as u32, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                EffectType::Delay { delay } => {
                    let feedback = params.delay_feedback.value();
                    let mix = params.delay_mix.value();
                    let time = params.delay_time.value();
                    let ducking = params.delay_ducking.value();
                    let ping_pong = params.delay_ping_pong.value();
                    delay.process(left, right, self.sample_rate, time, feedback, mix, ducking, ping_pong);
                }
                EffectType::Reverb { reverb } => {
                    let mix = params.reverb_mix.value();
                    let space = params.reverb_space.value();
                    let shimmer = params.reverb_shimmer.value();
                    let ducking = params.reverb_ducking.value();
                    reverb.process(left, right, self.sample_rate, mix, space, shimmer, ducking);
                }
                EffectType::Gate { gate } => {
                    let threshold = params.gate_threshold.value();
                    let attack = params.gate_attack.value();
                    let release = params.gate_release.value();
                    gate.process(left, right, self.sample_rate, threshold, attack, release);
                }
                EffectType::Bitcrusher { crusher } => {
                    let bits = params.bitcrush_bits.value();
                    let downsample = params.bitcrush_downsample.value();
                    let mix = params.bitcrush_mix.value();
                    let mode = params.bitcrush_mode.value();
                    crusher.process(left, right, bits, downsample, mix, mode);
                }
                EffectType::Overdrive { od } => {
                    let drive = params.overdrive_drive.value();
                    let tone = params.overdrive_tone.value();
                    let level = params.overdrive_level.value();
                    let algo = params.overdrive_algo.value();
                    od.process(left, right, drive, tone, level, algo);
                }
                EffectType::Eq { eq } => {
                    let low_freq = params.eq_low_freq.value();
                    let low_gain = params.eq_low_gain.value();
                    let mid_freq = params.eq_mid_freq.value();
                    let mid_gain = params.eq_mid_gain.value();
                    let mid_q = params.eq_mid_q.value();
                    let high_freq = params.eq_high_freq.value();
                    let high_gain = params.eq_high_gain.value();
                    eq.process(left, right, self.sample_rate, low_freq, low_gain, mid_freq, mid_gain, mid_q, high_freq, high_gain);
                }
            }
        }

        // Fade-in ramp after config updates to mask clicks
        self.apply_fade_ramp(left, right);
    }
}

// ParamResolver helper for looking up block-specific params
pub struct ParamResolver<'a> {
    params: &'a SlotsFxParams,
    block_params: &'a HashMap<String, f32>,
}

impl<'a> ParamResolver<'a> {
    pub fn new(params: &'a SlotsFxParams, block_params: &'a HashMap<String, f32>) -> Self {
        Self { params, block_params }
    }

    pub fn get_bool_for_block(&self, _sid: &str, id: &str, _global: bool) -> bool {
        if let Some(&val) = self.block_params.get(id) {
            return val > 0.5;
        }
        false
    }
}
