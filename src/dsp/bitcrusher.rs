pub struct Bitcrusher {
    phase: f32,
    held_l: f32,
    held_r: f32,
}

impl Bitcrusher {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            held_l: 0.0,
            held_r: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.held_l = 0.0;
        self.held_r = 0.0;
    }

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        bits: f32,
        downsample: f32,
        mix: f32,
        _mode: f32,
    ) {
        let quantize = 2.0_f32.powf(bits);
        let downsample_rate = downsample.max(1.0);

        for i in 0..left.len() {
            self.phase += 1.0;
            if self.phase >= downsample_rate {
                self.phase = 0.0;
                self.held_l = left[i];
                self.held_r = right[i];
            }

            let crushed_l = (self.held_l * quantize).round() / quantize;
            let crushed_r = (self.held_r * quantize).round() / quantize;

            left[i] = left[i] * (1.0 - mix) + crushed_l * mix;
            right[i] = right[i] * (1.0 - mix) + crushed_r * mix;
        }
    }
}
