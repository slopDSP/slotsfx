use super::pitch_detector::{
    frequency_to_note, quantize_to_scale, ScaleType, YinDetector,
};
use super::pitch_shifter::{PitchShifter, ShiftMode};

#[derive(Debug, Clone, Copy)]
pub struct AutoTuneConfig {
    pub enabled: bool,
    pub root_key: u8,
    pub scale: ScaleType,
    pub mode: ShiftMode,
    pub retune_speed: f32,
    pub correction_amount: f32,
}

impl Default for AutoTuneConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            root_key: 0,
            scale: ScaleType::Chromatic,
            mode: ShiftMode::PsoLa,
            retune_speed: 0.3,
            correction_amount: 1.0,
        }
    }
}

pub struct AutoTune {
    detector: YinDetector,
    shifter: PitchShifter,
    config: AutoTuneConfig,
    sample_rate: f32,
    detect_buf: Vec<f32>,
    detect_interval: usize,
    samples_since_detect: usize,
    target_freq: f32,
    current_shift: f32,
    current_period: f32,
}

impl AutoTune {
    pub fn new(sample_rate: f32, config: AutoTuneConfig) -> Self {
        Self {
            detector: YinDetector::new(sample_rate),
            shifter: PitchShifter::new(sample_rate, config.mode),
            config,
            sample_rate,
            detect_buf: Vec::with_capacity(2048),
            detect_interval: (sample_rate * 0.03) as usize,
            samples_since_detect: 0,
            target_freq: 0.0,
            current_shift: 0.0,
            current_period: 0.0,
        }
    }

    pub fn reset(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.detector.reset(sample_rate);
        self.shifter.reset(sample_rate, self.config.mode);
        self.detect_interval = (sample_rate * 0.03) as usize;
        self.detect_buf.clear();
        self.samples_since_detect = 0;
        self.target_freq = 0.0;
        self.current_shift = 0.0;
        self.current_period = 0.0;
    }

    pub fn update_config(&mut self, config: AutoTuneConfig) {
        let was_enabled = self.config.enabled;
        let mode_changed = config.mode != self.config.mode;
        self.config = config;
        if mode_changed {
            self.shifter.reset(self.sample_rate, config.mode);
        }
        if config.enabled && !was_enabled {
            self.detect_buf.clear();
            self.samples_since_detect = 0;
            self.current_shift = 0.0;
            self.target_freq = 0.0;
            self.current_period = 0.0;
        }
    }

    pub fn config(&self) -> &AutoTuneConfig {
        &self.config
    }

    pub fn current_shift_semitones(&self) -> f32 {
        self.current_shift
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32], channel: usize) {
        if !self.config.enabled || input.is_empty() {
            if !input.is_empty() && output.as_ptr() != input.as_ptr() {
                output[..input.len()].copy_from_slice(input);
            }
            return;
        }

        let len = input.len().min(output.len());

        // Only detect pitch on left channel (channel 0) to avoid double-detection
        if channel == 0 {
            // Accumulate samples for pitch detection
            for &s in input.iter().take(len) {
                self.detect_buf.push(s);
            }
            self.samples_since_detect += len;

            // Run pitch detection periodically
            if self.samples_since_detect >= self.detect_interval && self.detect_buf.len() >= 1024 {
                if let Some(pitch) = self.detector.detect(&self.detect_buf) {
                    let note = frequency_to_note(pitch.frequency, 440.0);
                    let target_note = quantize_to_scale(note.note, note.octave, self.config.root_key, self.config.scale);
                    let note_num = (note.octave as i16 * 12) + target_note as i16 - 57;
                    self.target_freq = 440.0 * 2.0_f32.powf(note_num as f32 / 12.0);
                    self.current_period = self.sample_rate / pitch.frequency;
                }
                self.detect_buf.clear();
                self.samples_since_detect = 0;
            }

            // Calculate required shift from target frequency
            let raw_shift = if self.target_freq > 0.0 && self.current_period > 0.0 {
                let avg_level: f32 = input.iter().take(len).map(|s| s.abs()).sum::<f32>() / len.max(1) as f32;
                if avg_level > 0.001 {
                    let detected_freq = self.sample_rate / self.current_period;
                    12.0 * (self.target_freq / detected_freq).log2()
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Smooth the shift
            let speed = (self.config.retune_speed * 0.3 + 0.001).min(0.5);
            self.current_shift += (raw_shift - self.current_shift) * speed;
            self.current_shift = self.current_shift.clamp(-12.0, 12.0);
        }

        // Apply pitch shifting with same shift on both channels
        self.shifter.process(input, output, self.current_period, self.current_shift);
    }
}
