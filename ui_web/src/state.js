// --- Global UI state ---
import { MODULE_COLORS } from './data.js';

export const state = {
  slot_knobs: {
    amp: ['amp_gain', 'amp_bass', 'amp_middle', 'amp_high', 'amp_output'],
    cab: ['cab_gain', 'cab_position', 'cab_size'],
    pitch: ['pitch_gain', 'pitch_semi', 'pitch_mix'],
    delay: ['delay_time', 'delay_feedback', 'delay_ducking', 'delay_mix'],
    verb: ['reverb_space', 'reverb_ducking', 'reverb_mix'],
    error: ['bitcrush_bits', 'bitcrush_downsample', 'bitcrush_mix'],
    od: ['overdrive_drive', 'overdrive_tone', 'overdrive_level'],
    eq: ['eq_low_gain', 'eq_mid_gain', 'eq_high_gain'],
    gate: ['gate_threshold', 'gate_attack', 'gate_release'],
  },
  routing_order: [],
  selected_slot_id: '',
  nam_model_index: 0,
  cab_ir_index: 0,
  is_profiling: false,
  colors: { ...MODULE_COLORS },
  input_gain: 0.0,
  output_gain: 0.0,
  pitch_semi: 0.0,
  active_snapshot_index: 0,
  snapshots: Array(8).fill(null).map(() => ({
    slots: [],
    params: {}
  })),
  macro_mappings: [],
  macros: [0.0, 0.0, 0.0, 0.0],
  self_instance_id: null,
  active_instances: [],
  ab_mode: 0,
  sweep_progress: 0,
  dragging_macro: null,
  dragging_param: null,
  dragging_param_slot_id: null,
  visualizers_enabled: true,
  visualizers_in_logo_menu: true,
  paired_sender_id: null,
  temp_capture: null,
  current_slot_peaks: null,
  // Tuner state
  tuner_note: '',
  tuner_cents: 0.0,
  tuner_active: false,
  // Auto-tune state
  auto_tune_enabled: false,
  auto_tune_key: 0,
  auto_tune_scale: 0,
  auto_tune_mode: 0, // 0 = Fast (PSOLA), 1 = Slow (Phase Vocoder)
  auto_tune_speed: 0.5,
  auto_tune_amount: 1.0,
};
