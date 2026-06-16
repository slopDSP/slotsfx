use std::f32::consts::PI;

use slotsfx::dsp::pitch_shifter::{PhaseVocoderShifter, PsoLaShifter};

const SAMPLE_RATE: f32 = 44100.0;
const DURATION_SECS: f32 = 2.0;
const BLOCK_SIZE: usize = 128;

fn write_wav(filename: &str, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(filename, spec).unwrap();
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        writer.write_sample(sample_i16).unwrap();
    }
    writer.finalize().unwrap();
}

fn analyze(name: &str, output: &[f32]) {
    let total = output.len();
    let non_zero = output.iter().filter(|&&s| s != 0.0).count();
    let max_val = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    // RMS energy in first 1/4 vs last 1/4
    let q1 = output.len() / 4;
    let rms_a: f32 = (output[..q1].iter().map(|s| s * s).sum::<f32>() / q1 as f32).sqrt();
    let rms_b: f32 = (output[3 * q1..].iter().map(|s| s * s).sum::<f32>() / q1 as f32).sqrt();
    eprintln!("{}: {} non-zero/{}, max={:.4}, RMS[0..{}]={:.4}, RMS[{}..]={:.4}",
              name, non_zero, total, max_val, q1, rms_a, 3*q1, rms_b);
    write_wav(&format!("test_{}.wav", name), output);
}

fn main() {
    let total_samples = (SAMPLE_RATE * DURATION_SECS) as usize;
    let input: Vec<f32> = (0..total_samples)
        .map(|i| (2.0 * PI * 440.0 * i as f32 / SAMPLE_RATE).sin() * 0.5)
        .collect();
    write_wav("test_input_440.wav", &input);
    eprintln!("Created test_input_440.wav");

    let mut output = vec![0.0f32; total_samples];

    // Test 1: PSOLA +12 semitones (one octave up)
    {
        let mut shifter = PsoLaShifter::new(SAMPLE_RATE);
        output.fill(0.0);
        let mut offset = 0usize;
        while offset < total_samples {
            let end = (offset + BLOCK_SIZE).min(total_samples);
            let inp = &input[offset..end];
            let out = &mut output[offset..end];
            shifter.process(inp, out, 12.0);
            offset = end;
        }
        analyze("psola_up12", &output);
    }

    // Test 2: PSOLA -12 semitones (one octave down)
    {
        let mut shifter = PsoLaShifter::new(SAMPLE_RATE);
        output.fill(0.0);
        let mut offset = 0usize;
        while offset < total_samples {
            let end = (offset + BLOCK_SIZE).min(total_samples);
            let inp = &input[offset..end];
            let out = &mut output[offset..end];
            shifter.process(inp, out, -12.0);
            offset = end;
        }
        analyze("psola_down12", &output);
    }

    // Test 3: PSOLA +24 semitones (two octaves up — worst case for buffer drain)
    {
        let mut shifter = PsoLaShifter::new(SAMPLE_RATE);
        output.fill(0.0);
        let mut offset = 0usize;
        while offset < total_samples {
            let end = (offset + BLOCK_SIZE).min(total_samples);
            let inp = &input[offset..end];
            let out = &mut output[offset..end];
            shifter.process(inp, out, 24.0);
            offset = end;
        }
        analyze("psola_up24", &output);
    }

    // Test 4: PhaseVocoder +12 semitones
    {
        let mut shifter = PhaseVocoderShifter::new(SAMPLE_RATE);
        output.fill(0.0);
        let mut offset = 0usize;
        while offset < total_samples {
            let end = (offset + BLOCK_SIZE).min(total_samples);
            let inp = &input[offset..end];
            let out = &mut output[offset..end];
            shifter.process(inp, out, 12.0);
            offset = end;
        }
        analyze("pvoc_up12", &output);
    }

    eprintln!("\nCompare test_*.wav in Audacity.");
}
