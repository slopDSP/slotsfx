pub fn generate_sweep(sample_rate: f32, duration_secs: f32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate) as usize;
    let mut sweep = vec![0.0f32; num_samples];
    let f1 = 20.0f32;
    let f2 = 20000.0f32;
    let ln_f2_f1 = (f2 / f1).ln();
    let factor = 2.0 * std::f32::consts::PI * f1 * duration_secs / ln_f2_f1;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let exponent = (t / duration_secs) * ln_f2_f1;
        let theta = factor * (exponent.exp() - 1.0);
        sweep[i] = theta.sin();
    }
    sweep
}

pub fn deconvolve_ir(recorded: &[f32], sample_rate: f32, duration_secs: f32) -> Vec<f32> {
    let sweep = generate_sweep(sample_rate, duration_secs);
    let ir_len = recorded.len().min(sweep.len());
    let mut ir = vec![0.0f32; ir_len];

    for i in 0..ir_len {
        ir[i] = recorded[i] - sweep[i];
    }
    ir
}
