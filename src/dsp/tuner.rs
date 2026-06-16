use super::pitch_detector::{frequency_to_note, YinDetector, NOTE_NAMES};

/// Shared tuner state accessible from the DSP thread
#[derive(Debug, Clone, Copy)]
pub struct TunerState {
    pub note: u8,
    pub octave: i8,
    pub cents: f32,
    pub frequency: f32,
    pub active: bool,
}

impl TunerState {
    pub fn note_name(&self) -> &'static str {
        if self.active {
            NOTE_NAMES.get(self.note as usize).unwrap_or(&"?")
        } else {
            "--"
        }
    }

    pub fn cents_display(&self) -> f32 {
        if self.active { self.cents } else { 0.0 }
    }
}

impl Default for TunerState {
    fn default() -> Self {
        Self {
            note: 0,
            octave: 0,
            cents: 0.0,
            frequency: 0.0,
            active: false,
        }
    }
}

/// The Tuner accumulates audio samples and periodically runs pitch detection
/// to determine the current note. Results are written to shared state for the UI.
pub struct Tuner {
    detector: YinDetector,
    sample_rate: f32,
    buffer: Vec<f32>,
    min_buffer_size: usize,
    max_buffer_size: usize,
    a4: f32,
    /// How often to run detection (in samples)
    detect_interval: usize,
    samples_accumulated: usize,
    /// Current tuner state
    current_state: TunerState,
}

impl Tuner {
    pub fn new(sample_rate: f32) -> Self {
        let min_size = 1024;
        let max_size = 4096;
        Self {
            detector: YinDetector::new(sample_rate),
            sample_rate,
            buffer: Vec::with_capacity(max_size),
            min_buffer_size: min_size,
            max_buffer_size: max_size,
            a4: 440.0,
            detect_interval: (sample_rate * 0.04) as usize,
            samples_accumulated: 0,
            current_state: TunerState::default(),
        }
    }

    pub fn reset(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.detector.reset(sample_rate);
        self.buffer.clear();
        self.samples_accumulated = 0;
        self.detect_interval = (sample_rate * 0.04) as usize;
        self.current_state = TunerState::default();
    }

    /// Feed audio samples into the tuner. Call this every audio block.
    pub fn feed_samples(&mut self, input: &[f32]) {
        for &sample in input {
            self.buffer.push(sample);
            if self.buffer.len() > self.max_buffer_size {
                let excess = self.buffer.len() - self.max_buffer_size;
                self.buffer.drain(..excess);
            }
        }
    }

    /// Process audio and return tuner state. Should be called every block.
    pub fn process(&mut self, input: &[f32]) -> &TunerState {
        self.feed_samples(input);
        self.samples_accumulated += input.len();

        if self.samples_accumulated >= self.detect_interval && self.buffer.len() >= self.min_buffer_size
        {
            self.samples_accumulated = 0;
            self.run_detection();
        }

        &self.current_state
    }

    fn run_detection(&mut self) {
        if self.buffer.len() < self.min_buffer_size {
            self.current_state.active = false;
            return;
        }

        let result = self.detector.detect(&self.buffer);
        match result {
            Some(pitch) if pitch.confidence > 0.05 => {
                let note_info = frequency_to_note(pitch.frequency, self.a4);
                self.current_state = TunerState {
                    note: note_info.note,
                    octave: note_info.octave,
                    cents: note_info.cents,
                    frequency: pitch.frequency,
                    active: true,
                };
            }
            _ => {
                self.current_state.active = false;
            }
        }
    }

    pub fn state(&self) -> &TunerState {
        &self.current_state
    }
}
