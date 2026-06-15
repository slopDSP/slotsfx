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

        // RBJ audio EQ cookbook low shelf filter
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        self.a0 = b0 / a0; self.a1 = b1 / a0; self.a2 = b2 / a0;
        self.b1 = a1 / a0; self.b2 = a2 / a0;
    }

    pub fn set_high_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() * 0.707;
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();
        let sqrt_a = a.sqrt();

        // RBJ audio EQ cookbook high shelf filter
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha;

        self.a0 = b0 / a0; self.a1 = b1 / a0; self.a2 = b2 / a0;
        self.b1 = a1 / a0; self.b2 = a2 / a0;
    }

    pub fn set_peaking(&mut self, sample_rate: f32, freq: f32, gain_db: f32, q: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = w0.sin() * 0.5 * q;
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();

        // RBJ audio EQ cookbook peaking filter
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        self.a0 = b0 / a0; self.a1 = b1 / a0; self.a2 = b2 / a0;
        self.b1 = a1 / a0; self.b2 = a2 / a0;
    }
}

pub struct ParametricEq {
    lowshelf_l: Biquad,
    peaking_l: Biquad,
    highshelf_l: Biquad,
    lowshelf_r: Biquad,
    peaking_r: Biquad,
    highshelf_r: Biquad,
}

impl ParametricEq {
    pub fn new() -> Self {
        Self {
            lowshelf_l: Biquad::new(),
            peaking_l: Biquad::new(),
            highshelf_l: Biquad::new(),
            lowshelf_r: Biquad::new(),
            peaking_r: Biquad::new(),
            highshelf_r: Biquad::new(),
        }
    }

    pub fn reset(&mut self) {
        self.lowshelf_l.reset();
        self.peaking_l.reset();
        self.highshelf_l.reset();
        self.lowshelf_r.reset();
        self.peaking_r.reset();
        self.highshelf_r.reset();
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
        // Update coefficients for both channels (same filter, independent state)
        self.lowshelf_l.set_low_shelf(sample_rate, low_freq, low_gain);
        self.peaking_l.set_peaking(sample_rate, mid_freq, mid_gain, mid_q);
        self.highshelf_l.set_high_shelf(sample_rate, high_freq, high_gain);
        self.lowshelf_r.set_low_shelf(sample_rate, low_freq, low_gain);
        self.peaking_r.set_peaking(sample_rate, mid_freq, mid_gain, mid_q);
        self.highshelf_r.set_high_shelf(sample_rate, high_freq, high_gain);

        for i in 0..left.len() {
            left[i] = self.lowshelf_l.process_sample(left[i]);
            left[i] = self.peaking_l.process_sample(left[i]);
            left[i] = self.highshelf_l.process_sample(left[i]);

            right[i] = self.lowshelf_r.process_sample(right[i]);
            right[i] = self.peaking_r.process_sample(right[i]);
            right[i] = self.highshelf_r.process_sample(right[i]);
        }
    }
}
