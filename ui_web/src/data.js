// --- Data definitions shared across all UI modules ---

// Parameter specs: min, max, type, units
export const paramSpecs = {
  amp_gain: { min: -12.0, max: 12.0, type: 'db', label: 'Gain' },
  amp_bass: {
    min: 0.0, max: 1.0, type: 'percent', label: 'Bass',
    subParams: { freq: { paramId: 'amp_bass_freq', label: '', min: 80, max: 400, default: 150 } }
  },
  amp_middle: {
    min: 0.0, max: 1.0, type: 'percent', label: 'Middle',
    subParams: { freq: { paramId: 'amp_mid_freq', label: '', min: 200, max: 2000, default: 425 } }
  },
  amp_high: {
    min: 0.0, max: 1.0, type: 'percent', label: 'High',
    subParams: { freq: { paramId: 'amp_high_freq', label: '', min: 1000, max: 8000, default: 1800 } }
  },
  amp_output: { min: -12.0, max: 12.0, type: 'db', label: 'Output' },

  amp_bass_freq: { min: 80.0, max: 400.0, type: 'hz', label: 'Bass Freq' },
  amp_mid_freq: { min: 200.0, max: 2000.0, type: 'hz', label: 'Mid Freq' },
  amp_high_freq: { min: 1000.0, max: 8000.0, type: 'hz', label: 'High Freq' },

  cab_gain: { min: -12.0, max: 12.0, type: 'db', label: 'Gain' },
  cab_position: { min: 0.0, max: 1.0, type: 'percent', label: 'Position' },
  cab_size: { min: 0.0, max: 1.0, type: 'percent', label: 'Size' },

  pitch_gain: { min: -12.0, max: 12.0, type: 'db', label: 'Gain' },
  pitch_semi: { min: -12.0, max: 12.0, type: 'semi', label: 'Semitones' },
  pitch_mix: { min: 0.0, max: 1.0, type: 'percent', label: 'Mix' },

  delay_mix: { min: 0.0, max: 1.0, type: 'percent', label: 'Mix' },
  delay_feedback: { min: 0.0, max: 1.0, type: 'percent', label: 'Feedback' },
  delay_time: { min: 50.0, max: 1000.0, type: 'ms', label: 'Time' },
  delay_ducking: { min: 0.0, max: 1.0, type: 'percent', label: 'Ducking' },
  delay_ping_pong: { min: 0.0, max: 2.0, type: 'float', label: 'Ping Pong' },

  reverb_mix: { min: 0.0, max: 1.0, type: 'percent', label: 'Mix' },
  reverb_space: { min: 0.0, max: 1.0, type: 'percent', label: 'Space' },
  reverb_shimmer: { min: 0.0, max: 1.0, type: 'percent', label: 'Shimmer' },
  reverb_ducking: { min: 0.0, max: 1.0, type: 'percent', label: 'Ducking' },

  bitcrush_bits: { min: 1.0, max: 24.0, type: 'float', label: 'Bits' },
  bitcrush_downsample: { min: 1.0, max: 32.0, type: 'float', label: 'Downsample' },
  bitcrush_mix: { min: 0.0, max: 1.0, type: 'percent', label: 'Mix' },
  bitcrush_mode: { min: 0.0, max: 2.0, type: 'float', label: 'Mode' },

  overdrive_drive: { min: 1.0, max: 100.0, type: 'float', label: 'Drive' },
  overdrive_tone: { min: 0.0, max: 1.0, type: 'percent', label: 'Tone' },
  overdrive_level: { min: 0.0, max: 1.0, type: 'percent', label: 'Level' },
  overdrive_algo: { min: 0.0, max: 2.0, type: 'float', label: 'Algo' },

  eq_low_gain: {
    min: -12.0, max: 12.0, type: 'db', label: 'Low Gain',
    subParams: { freq: { paramId: 'eq_low_freq', label: 'Low Freq', min: 20, max: 1000, default: 100 } }
  },
  eq_low_freq: { min: 20.0, max: 1000.0, type: 'hz', label: 'Low Freq' },
  eq_mid_gain: {
    min: -12.0, max: 12.0, type: 'db', label: 'Mid Gain',
    subParams: {
      freq: { paramId: 'eq_mid_freq', label: 'Mid Freq', min: 200, max: 5000, default: 1000 },
      q: { paramId: 'eq_mid_q', label: 'Mid Q', min: 0.1, max: 10.0, default: 1.0 }
    }
  },
  eq_mid_freq: { min: 200.0, max: 5000.0, type: 'hz', label: 'Mid Freq' },
  eq_mid_q: { min: 0.1, max: 10.0, type: 'float', label: 'Mid Q' },
  eq_high_gain: {
    min: -12.0, max: 12.0, type: 'db', label: 'High Gain',
    subParams: { freq: { paramId: 'eq_high_freq', label: 'High Freq', min: 1000, max: 20000, default: 5000 } }
  },
  eq_high_freq: { min: 1000.0, max: 20000.0, type: 'hz', label: 'High Freq' },

  gate_threshold: { min: -60.0, max: 0.0, type: 'db', label: 'Threshold' },
  gate_attack: { min: 0.1, max: 50.0, type: 'ms', label: 'Attack' },
  gate_release: { min: 10.0, max: 500.0, type: 'ms', label: 'Release' },

  auto_tune_toggle: { min: 0.0, max: 1.0, type: 'bool', label: 'Auto-Tune' },
  auto_tune_key: { min: 0.0, max: 11.0, type: 'float', label: 'Key' },
  auto_tune_scale: { min: 0.0, max: 2.0, type: 'float', label: 'Scale' },
  auto_tune_mode: { min: 0.0, max: 1.0, type: 'bool', label: 'AT Mode' },
  auto_tune_speed: { min: 0.0, max: 1.0, type: 'percent', label: 'Retune' },
  auto_tune_amount: { min: 0.0, max: 1.0, type: 'percent', label: 'Amount' },
  macro_1: { min: 0.0, max: 1.0, type: 'percent', label: 'Macro 1' },
  macro_2: { min: 0.0, max: 1.0, type: 'percent', label: 'Macro 2' },
  macro_3: { min: 0.0, max: 1.0, type: 'percent', label: 'Macro 3' },
  macro_4: { min: 0.0, max: 1.0, type: 'percent', label: 'Macro 4' },
};

// Module definitions: which knobs each slot type has
export const modulesData = {
  amp: {
    id: 'amp',
    title: 'Amp',
    desc: 'LOAD PROFILE',
    knobs: ['amp_gain', 'amp_bass', 'amp_middle', 'amp_high', 'amp_output'],
  },
  cab: {
    id: 'cab',
    title: 'Cab',
    desc: 'LOAD IR',
    knobs: ['cab_gain', 'cab_position', 'cab_size'],
  },
  pitch: {
    id: 'pitch',
    title: 'Pitch Shifter',
    desc: 'LOW-LATENCY PITCH SHIFT',
    knobs: ['pitch_gain', 'pitch_semi', 'pitch_mix'],
  },
  delay: {
    id: 'delay',
    title: 'Delay',
    desc: 'PING-PONG / DUCKABLE ECHO',
    knobs: ['delay_time', 'delay_feedback', 'delay_ducking', 'delay_mix'],
  },
  verb: {
    id: 'verb',
    title: 'Reverb',
    desc: 'DUCKABLE SCHROEDER VERB',
    knobs: ['reverb_space', 'reverb_ducking', 'reverb_mix'],
  },
  shimmer: {
    id: 'shimmer',
    title: 'Cosmos',
    desc: 'SHIMMER VERB',
    knobs: ['reverb_mix', 'reverb_space', 'reverb_shimmer'],
  },
  gate: {
    id: 'gate',
    title: 'Gate',
    desc: 'NOISE GATE',
    knobs: ['gate_threshold', 'gate_attack', 'gate_release'],
  },
  error: {
    id: 'error',
    title: 'Error',
    desc: 'BIT CRUSHER & SHAPER',
    knobs: ['bitcrush_bits', 'bitcrush_downsample', 'bitcrush_mix'],
  },
  od: {
    id: 'od',
    title: 'OD',
    desc: 'ANALOG OVERDRIVE EMULATOR',
    knobs: ['overdrive_drive', 'overdrive_tone', 'overdrive_level'],
  },
  eq: {
    id: 'eq',
    title: 'EQ',
    desc: 'PARAMETRIC EQ',
    knobs: ['eq_low_gain', 'eq_mid_gain', 'eq_high_gain'],
  },
};

// Slot type → CSS color mapping
export const MODULE_COLORS = {
  amp: '#E07A5F',
  cab: '#E07A5F',
  pitch: '#00D2FF',
  delay: '#00ccff',
  shimmer: '#A27DDF',
  gate: '#4ADE80',
  error: '#ff4545',
  od: '#ffaa00',
  eq: '#00ffaa',
  verb: '#b666ff',
};
