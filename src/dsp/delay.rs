pub struct Delay {
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    pos: usize,
}

impl Delay {
    pub fn new() -> Self {
        Self {
            buffer_l: vec![0.0; 48000],
            buffer_r: vec![0.0; 48000],
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
        time_ms: f32,
        feedback: f32,
        mix: f32,
        ducking: f32,
        ping_pong: bool,
    ) {
        let delay_samples = ((time_ms / 1000.0) * sample_rate) as usize;
        let delay_samples = delay_samples.min(self.buffer_l.len() - 1).max(1);

        for i in 0..left.len() {
            let read_pos = if self.pos >= delay_samples {
                self.pos - delay_samples
            } else {
                self.buffer_l.len() - (delay_samples - self.pos)
            };

            let wet_l = self.buffer_l[read_pos];
            let wet_r = if ping_pong {
                self.buffer_r[if read_pos > 0 { read_pos - 1 } else { self.buffer_r.len() - 1 }]
            } else {
                self.buffer_r[read_pos]
            };

            self.buffer_l[self.pos] = left[i] + wet_l * feedback * (1.0 - ducking);
            self.buffer_r[self.pos] = right[i] + wet_r * feedback * (1.0 - ducking);

            left[i] = left[i] * (1.0 - mix) + wet_l * mix;
            right[i] = right[i] * (1.0 - mix) + wet_r * mix;

            self.pos += 1;
            if self.pos >= self.buffer_l.len() {
                self.pos = 0;
            }
        }
    }
}
