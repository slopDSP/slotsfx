pub struct Overdrive;

impl Overdrive {
    pub fn new() -> Self {
        Self
    }

    pub fn reset(&mut self) {}

    pub fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        drive: f32,
        tone: f32,
        level: f32,
        algo: f32,
    ) {
        let drive_amt = drive / 50.0;
        for i in 0..left.len() {
            let algo_idx = algo.round() as u32;
            let process = |sample: f32| -> f32 {
                let driven = sample * drive_amt;
                let clipped = match algo_idx {
                    0 => (driven).tanh(),
                    1 => {
                        if driven > 1.0 { 1.0 }
                        else if driven < -1.0 { -1.0 }
                        else { driven * (1.5 - driven * driven * 0.5) }
                    }
                    _ => {
                        if driven > 1.5 { 1.0 }
                        else if driven < -1.5 { -1.0 }
                        else { driven * (1.0 - driven * driven / 6.0) }
                    }
                };
                let toned = clipped * (1.0 - tone * 0.3) + driven * tone * 0.3;
                toned * level * 2.0
            };
            left[i] = process(left[i]);
            right[i] = process(right[i]);
        }
    }
}
