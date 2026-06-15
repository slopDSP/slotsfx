pub struct Biquad {
    a0: f32, a1: f32, a2: f32,
    b1: f32, b2: f32,
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl Biquad {
    pub fn new() -> Self {
        Self {
            a0: 1.0, a1: 0.0, a2: 0.0,
            b1: 0.0, b2: 0.0,
            x1: 0.0, x2: 0.0,
            y1: 0.0, y2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0;
        self.y1 = 0.0; self.y2 = 0.0;
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.a0 * input + self.a1 * self.x1 + self.a2 * self.x2
            - self.b1 * self.y1 - self.b2 * self.y2;
        self.x2 = self.x1; self.x1 = input;
        self.y2 = self.y1; self.y1 = output;
        output
    }

    pub fn set_low_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() * 0.707;
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();
        let sqrt_a = a.sqrt();

        self.b1 = -2.0 * cos_w0;
        self.b2 = 1.0 - alpha;
        // Standard low shelf coefficients
        self.a0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        self.a1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        self.a2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        // Normalize
        let norm = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        self.a0 /= norm; self.a1 /= norm; self.a2 /= norm;
        self.b1 = -2.0 * cos_w0 / norm;
        self.b2 = (1.0 - alpha) / norm;
    }

    pub fn set_high_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() * 0.707;
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();
        let sqrt_a = a.sqrt();

        let norm = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        self.a0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha) / norm;
        self.a1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0) / norm;
        self.a2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / norm;
        self.b1 = 2.0 * cos_w0 / norm;
        self.b2 = ((a - 1.0) + (a + 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / norm;
    }

    pub fn set_peaking(&mut self, sample_rate: f32, freq: f32, gain_db: f32, q: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() * 0.5 * q;
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();

        let norm = 1.0 + alpha / a;
        self.a0 = (1.0 + alpha * a) / norm;
        self.a1 = -2.0 * cos_w0 / norm;
        self.a2 = (1.0 - alpha * a) / norm;
        self.b1 = 2.0 * cos_w0 / norm;
        self.b2 = (alpha / a - 1.0) / norm;
    }
}

pub struct ParametricEq {
    lowshelf: Biquad,
    peaking: Biquad,
    highshelf: Biquad,
}

impl ParametricEq {
    pub fn new() -> Self {
        Self {
            lowshelf: Biquad::new(),
            peaking: Biquad::new(),
            highshelf: Biquad::new(),
        }
    }

    pub fn reset(&mut self) {
        self.lowshelf.reset();
        self.peaking.reset();
        self.highshelf.reset();
    }

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        sample_rate: f32,
        low_freq: f32,
        low_gain: f32,
        mid_freq: f32,
        mid_gain: f32,
        mid_q: f32,
        high_freq: f32,
        high_gain: f32,
    ) {
        self.lowshelf.set_low_shelf(sample_rate, low_freq, low_gain);
        self.peaking.set_peaking(sample_rate, mid_freq, mid_gain, mid_q);
        self.highshelf.set_high_shelf(sample_rate, high_freq, high_gain);

        for i in 0..left.len() {
            left[i] = self.lowshelf.process_sample(left[i]);
            left[i] = self.peaking.process_sample(left[i]);
            left[i] = self.highshelf.process_sample(left[i]);
        }
        for i in 0..right.len() {
            right[i] = self.lowshelf.process_sample(right[i]);
            right[i] = self.peaking.process_sample(right[i]);
            right[i] = self.highshelf.process_sample(right[i]);
        }
    }
}
