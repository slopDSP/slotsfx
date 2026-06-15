pub struct NoiseGate {
    envelope: f32,
}

impl NoiseGate {
    pub fn new() -> Self {
        Self {
            envelope: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        sample_rate: f32,
        threshold_db: f32,
        attack_ms: f32,
        release_ms: f32,
    ) {
        let threshold = 10.0_f32.powf(threshold_db / 20.0);
        let attack_coeff = 1.0 - (-1.0 / (attack_ms / 1000.0 * sample_rate)).exp();
        let release_coeff = 1.0 - (-1.0 / (release_ms / 1000.0 * sample_rate)).exp();

        for i in 0..left.len() {
            let input_level = (left[i].abs() + right[i].abs()) * 0.5;

            let target = if input_level > threshold { 1.0 } else { 0.0 };
            let coeff = if target > self.envelope { attack_coeff } else { release_coeff };
            self.envelope += coeff * (target - self.envelope);

            left[i] *= self.envelope;
            right[i] *= self.envelope;
        }
    }
}
