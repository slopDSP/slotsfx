pub struct Reverb {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    pos: usize,
}

impl Reverb {
    pub fn new() -> Self {
        Self {
            buffer_l: vec![0.0; 96000],
            buffer_r: vec![0.0; 96000],
            pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.pos = 0;
    }

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        sample_rate: f32,
        mix: f32,
        space: f32,
        shimmer: f32,
        ducking: f32,
    ) {
        let decay_len = (space * 0.5 * sample_rate) as usize;
        let decay_len = decay_len.max(1000).min(self.buffer_l.len() - 1);

        for i in 0..left.len() {
            let read_pos = if self.pos >= decay_len {
                self.pos - decay_len
            } else {
                self.buffer_l.len() - (decay_len - self.pos)
            };

            let decay = 0.5 * (1.0 - space * 0.5);
            let wet_l = self.buffer_l[read_pos] * decay;
            let wet_r = self.buffer_r[read_pos] * decay;

            let shimmer_amount = shimmer * 0.3;
            let shimmer_l = wet_l * (1.0 + shimmer_amount);
            let shimmer_r = wet_r * (1.0 + shimmer_amount);

            self.buffer_l[self.pos] = (left[i] + shimmer_l) * (1.0 - ducking);
            self.buffer_r[self.pos] = (right[i] + shimmer_r) * (1.0 - ducking);

            left[i] = left[i] * (1.0 - mix) + shimmer_l * mix;
            right[i] = right[i] * (1.0 - mix) + shimmer_r * mix;

            self.pos += 1;
            if self.pos >= self.buffer_l.len() {
                self.pos = 0;
            }
        }
    }
}
