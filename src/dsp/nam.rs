pub struct NamBlock {
    model_l: Option<nam_rs::Model>,
    model_r: Option<nam_rs::Model>,
    loudness: Option<f32>,
}

impl NamBlock {
    pub fn new() -> Self {
        Self {
            model_l: None,
            model_r: None,
            loudness: None,
        }
    }

    pub fn set_models(&mut self, l: Option<nam_rs::Model>, r: Option<nam_rs::Model>, loudness: Option<f32>) {
        self.model_l = l;
        self.model_r = r;
        self.loudness = loudness;
    }

    pub fn is_loaded(&self) -> bool {
        self.model_l.is_some()
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], gain_db: f32) {
        let gain = 10.0_f32.powf(gain_db / 20.0);

        // Apply gain before passing to the model
        if gain != 1.0 {
            for s in left.iter_mut() { *s *= gain; }
            for s in right.iter_mut() { *s *= gain; }
        }

        // Process the entire buffer at once — not one sample at a time
        if let Some(ref mut model) = self.model_l {
            model.process_buffer(left);
        }
        if let Some(ref mut model) = self.model_r {
            model.process_buffer(right);
        }

        // Loudness normalization — disabled: LUFS-to-linear is 10^(LUFS/20),
        // not 1/LUFS. The model's gain staging is handled by amp_gain instead.
        // (Removing this is a regression fix: the previous 1.0/loudness formula
        //  inverted the signal for typical LUFS values like -20.)
    }
}
