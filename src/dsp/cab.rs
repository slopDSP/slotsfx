pub struct CabConvolver {
    ir_l: Vec<f32>,
    ir_r: Vec<f32>,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    pos: usize,
    /// Peak of the original IR (for normalization)
    ir_peak: f32,
    /// Current normalization: true = IR samples are pre-scaled by 1/ir_peak, false = raw IR
    normalize: bool,
}

impl CabConvolver {
    pub fn new() -> Self {
        Self {
            ir_l: Vec::new(),
            ir_r: Vec::new(),
            buffer_l: Vec::new(),
            buffer_r: Vec::new(),
            pos: 0,
            ir_peak: 1.0,
            normalize: false,
        }
    }

    pub fn set_ir(&mut self, left: Vec<f32>, right: Vec<f32>, normalize: bool) {
        // Compute peak of the raw IR
        let mut peak = 0.0f32;
        for &s in &left { peak = peak.max(s.abs()); }
        for &s in &right { peak = peak.max(s.abs()); }
        self.ir_peak = peak.max(1e-8);

        // Apply normalization: scale IR samples so peak = 1.0
        let scale = if normalize { 1.0 / self.ir_peak } else { 1.0 };
        self.normalize = normalize;

        self.ir_l = left.iter().map(|&s| s * scale).collect();
        self.ir_r = right.iter().map(|&s| s * scale).collect();

        self.buffer_l = vec![0.0; self.ir_l.len()];
        self.buffer_r = vec![0.0; self.ir_r.len()];
        self.pos = 0;
    }

    pub fn is_loaded(&self) -> bool {
        !self.ir_l.is_empty()
    }

    /// Toggle normalization after IR is loaded. Re-scales IR samples in-place.
    pub fn set_normalize(&mut self, normalize: bool) {
        if self.ir_l.is_empty() || self.normalize == normalize {
            return;
        }
        // Current IR is already in some scale; we need to undo the old scale
        // and apply the new one. Since we stored scaled values, the "base" peak
        // is what we called ir_peak. Current scale = (normalize ? 1/ir_peak : 1.0).
        // To switch: multiply by (new_scale / old_scale).
        let old_scale = if self.normalize { 1.0 / self.ir_peak } else { 1.0 };
        let new_scale = if normalize { 1.0 / self.ir_peak } else { 1.0 };
        let rel_scale = new_scale / old_scale;

        for s in self.ir_l.iter_mut() { *s *= rel_scale; }
        for s in self.ir_r.iter_mut() { *s *= rel_scale; }
        self.normalize = normalize;
    }

    /// position: 0.0 = close mic (direct), 1.0 = far mic (delayed start)
    /// size: 0.0 = small room (faster decay), 1.0 = large room (longer tail)
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], gain_db: f32, position: f32, size: f32) {
        if self.ir_l.is_empty() {
            return;
        }
        let gain = 10.0_f32.powf(gain_db / 20.0);
        let ir_len = self.ir_l.len();

        // Pre-delay: position 0→1 maps to 0→50ms delay at 44100 Hz
        let pre_delay_samples = (position * 50.0 * 44100.0 / 1000.0) as usize;
        let pre_delay_clamped = pre_delay_samples.min(ir_len.saturating_sub(1));

        // Tail fade: size 0→1 maps to tail starting at 10%→50% of IR length
        let tail_start = (ir_len as f32 * (0.10 + size * 0.40)) as usize;
        let tail_start = tail_start.min(ir_len.saturating_sub(1));

        for i in 0..left.len() {
            self.buffer_l[self.pos] = left[i] * gain;
            self.buffer_r[self.pos] = right[i] * gain;

            let mut out_l = 0.0f32;
            let mut out_r = 0.0f32;
            let mut idx = self.pos;
            for j in 0..ir_len {
                if j < pre_delay_clamped {
                    // Pre-delay region: no contribution
                } else {
                    let tail_fade = if j >= tail_start {
                        let t = (j - tail_start) as f32 / (ir_len - tail_start) as f32;
                        let min_level = 0.3 + size * 0.7;
                        let cos_t = (std::f32::consts::PI * t * 0.5).cos();
                        min_level + (1.0 - min_level) * cos_t
                    } else {
                        1.0
                    };

                    out_l += self.buffer_l[idx] * self.ir_l[j] * tail_fade;
                    out_r += self.buffer_r[idx] * self.ir_r[j] * tail_fade;
                }

                if idx == 0 {
                    idx = ir_len - 1;
                } else {
                    idx -= 1;
                }
            }

            left[i] = out_l;
            right[i] = out_r;

            self.pos += 1;
            if self.pos >= ir_len {
                self.pos = 0;
            }
        }
    }
}
