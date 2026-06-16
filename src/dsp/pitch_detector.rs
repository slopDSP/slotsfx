pub const NOTE_NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

#[derive(Debug, Clone, Copy)]
pub struct PitchResult {
    pub frequency: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct NoteInfo {
    pub note: u8,
    pub octave: i8,
    pub cents: f32,
    pub frequency: f32,
    pub confidence: f32,
}

/// YIN pitch detection algorithm
/// Based on: De Cheveigné & Kawahara (2002)
pub struct YinDetector {
    buffer_size: usize,
    sample_rate: f32,
    threshold: f32,
    yin_buffer: Vec<f32>,
    min_freq: f32,
    max_freq: f32,
}

impl YinDetector {
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = 2048;
        Self {
            buffer_size,
            sample_rate,
            threshold: 0.15,
            yin_buffer: vec![0.0; buffer_size / 2 + 1],
            min_freq: 30.0,
            max_freq: 2000.0,
        }
    }

    pub fn reset(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn detect(&mut self, buffer: &[f32]) -> Option<PitchResult> {
        let len = buffer.len().min(self.buffer_size);
        if len < 20 {
            return None;
        }

        let half = len / 2;
        if half > self.yin_buffer.len() {
            self.yin_buffer.resize(half, 0.0);
        }

        // Tau limits from min/max frequency
        let tau_min = (self.sample_rate / self.max_freq).ceil() as usize;
        let tau_max = (self.sample_rate / self.min_freq).floor() as usize;
        let tau_max = tau_max.min(half - 1);

        if tau_max <= tau_min {
            return None;
        }

        // Step 1: Difference function
        for tau in 0..half {
            let mut diff = 0.0;
            for i in 0..(half - tau).min(len) {
                let d = buffer[i] - buffer[i + tau];
                diff += d * d;
            }
            self.yin_buffer[tau] = diff;
        }

        // Step 2: Cumulative mean normalized difference function
        let mut running_sum = 0.0;
        self.yin_buffer[0] = 1.0;
        for tau in 1..half {
            running_sum += self.yin_buffer[tau];
            if running_sum > 0.0 {
                self.yin_buffer[tau] = self.yin_buffer[tau] * tau as f32 / running_sum;
            } else {
                self.yin_buffer[tau] = 1.0;
            }
        }

        // Step 3: Absolute threshold - find first minimum below threshold
        let mut tau_idx = 0;
        let mut below_threshold = false;
        for t in (tau_min + 1)..tau_max {
            if self.yin_buffer[t] < self.threshold
                && self.yin_buffer[t] < self.yin_buffer[t - 1]
                && self.yin_buffer[t] < self.yin_buffer[t + 1]
            {
                tau_idx = t;
                below_threshold = true;
                break;
            }
        }

        // If nothing below threshold, find global minimum
        if !below_threshold {
            let mut min_val = f32::MAX;
            for t in tau_min..tau_max {
                if self.yin_buffer[t] < min_val {
                    min_val = self.yin_buffer[t];
                    tau_idx = t;
                }
            }
            if min_val >= 1.0 {
                return None;
            }
        }

        // Step 4: Parabolic interpolation for sub-sample accuracy
        let tau_f = if tau_idx > 0 && tau_idx < half - 1 {
            let y0 = self.yin_buffer[tau_idx - 1];
            let y1 = self.yin_buffer[tau_idx];
            let y2 = self.yin_buffer[tau_idx + 1];
            let a = (y0 + y2 - 2.0 * y1) / 2.0;
            let b = (y2 - y0) / 2.0;
            if a.abs() > 1e-12 {
                tau_idx as f32 - b / (2.0 * a)
            } else {
                tau_idx as f32
            }
        } else {
            tau_idx as f32
        };

        if tau_f <= 0.0 {
            return None;
        }

        let frequency = self.sample_rate / tau_f;
        let confidence = 1.0 - self.yin_buffer[tau_idx].min(1.0);

        if frequency < self.min_freq || frequency > self.max_freq || confidence < 0.05 {
            return None;
        }

        Some(PitchResult {
            frequency,
            confidence,
        })
    }
}

pub fn frequency_to_note(freq: f32, a4: f32) -> NoteInfo {
    if freq <= 0.0 {
        return NoteInfo {
            note: 0,
            octave: 0,
            cents: 0.0,
            frequency: 0.0,
            confidence: 0.0,
        };
    }

    // MIDI note 69 = A4 = 440Hz
    let midi_note_f = 69.0 + 12.0 * (freq / a4).log2();
    let midi_note = (midi_note_f + 0.5).floor() as i16;
    let note = ((midi_note % 12 + 12) % 12) as u8;
    let octave = (midi_note / 12 - 1) as i8;
    let target_freq = a4 * 2.0_f32.powf((midi_note - 69) as f32 / 12.0);
    let cents = 1200.0 * (freq / target_freq).log2();

    NoteInfo {
        note,
        octave,
        cents,
        frequency: freq,
        confidence: 0.0,
    }
}

/// Scale definitions for auto-tune
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleType {
    Chromatic,
    Major,
    Minor,
}

impl ScaleType {
    pub fn intervals(&self) -> &[u8] {
        match self {
            ScaleType::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            ScaleType::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleType::Minor => &[0, 2, 3, 5, 7, 8, 10],
        }
    }
}

/// Quantize a note to the nearest scale degree
pub fn quantize_to_scale(note: u8, _octave: i8, root_key: u8, scale: ScaleType) -> u8 {
    let intervals = scale.intervals();
    let note_in_key = ((note as i16 - root_key as i16) % 12 + 12) % 12;

    let mut best_distance = 12i16;
    let mut best_target = note;
    for &interval in intervals {
        let target = (root_key as i16 + interval as i16) % 12;
        let dist = ((note_in_key as i16 - interval as i16) % 12 + 12) % 12;
        let dist_rev = ((interval as i16 - note_in_key as i16) % 12 + 12) % 12;
        let d = dist.min(dist_rev);
        if d < best_distance {
            best_distance = d;
            best_target = target as u8;
        }
    }
    best_target
}

pub fn frequency_to_semitone_distance(freq: f32, target_freq: f32) -> f32 {
    if freq <= 0.0 || target_freq <= 0.0 {
        return 0.0;
    }
    12.0 * (freq / target_freq).log2()
}
