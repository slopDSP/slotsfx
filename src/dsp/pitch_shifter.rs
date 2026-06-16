use std::f32::consts::PI;
use rustfft::{FftPlanner, num_complex::Complex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShiftMode {
    PsoLa,
    PhaseVocoder,
}

/// Granular pitch shifter using overlap-add (Fast mode).
///
/// Uses a ring buffer for input with absolute sample counters to safely
/// handle wrapping. Grains are taken from the input at intervals of `hop_in`
/// and placed in the output at intervals of `hop_out = hop_in / shift`.
///
/// Pitch up (shift>1): hop_out < hop_in → grains overlap MORE → frequency ↑
/// Pitch down(shift<1): hop_out > hop_in → grains overlap LESS → frequency ↓
pub struct PsoLaShifter {
    sample_rate: f32,
    buf: Vec<f32>,
    buf_len: usize,
    write_pos: usize,
    grain_size: usize,
    hop_in: usize,
    window: Vec<f32>,
    out_buf: Vec<f32>,
    out_buf_len: usize,
    out_read: usize,
    write_out: usize,
    total_written: u64,
    total_read: u64,
    grain_phase: f32,
}

impl PsoLaShifter {
    pub fn new(sample_rate: f32) -> Self {
        let grain_size = 512;
        let hop_in = grain_size / 4;
        let buf_len = (sample_rate * 0.15) as usize;
        let out_buf_len = grain_size * 4;
        let latency = grain_size;

        Self {
            sample_rate,
            buf: vec![0.0; buf_len],
            buf_len,
            write_pos: 0,
            grain_size,
            hop_in,
            window: Self::make_hanning(grain_size),
            out_buf: vec![0.0; out_buf_len],
            out_buf_len,
            out_read: 0,
            write_out: latency % out_buf_len,
            total_written: 0,
            total_read: 0,
            grain_phase: 0.0,
        }
    }

    fn make_hanning(size: usize) -> Vec<f32> {
        (0..size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos()))
            .collect()
    }

    pub fn reset(&mut self, sample_rate: f32) {
        *self = Self::new(sample_rate);
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32], shift_semitones: f32) {
        let len = input.len().min(output.len());

        if shift_semitones.abs() < 0.5 {
            output[..len].copy_from_slice(&input[..len]);
            return;
        }

        let shift = 2.0_f32.powf(shift_semitones / 12.0);
        let hop_out = self.hop_in as f32 / shift;
        let hop_out_int = (hop_out.round() as usize).max(1);

        for i in 0..len {
            // Write input to ring buffer
            self.buf[self.write_pos] = input[i];
            self.write_pos = (self.write_pos + 1) % self.buf_len;
            self.total_written += 1;

            // Read output
            output[i] = self.out_buf[self.out_read];
            self.out_buf[self.out_read] = 0.0;
            self.out_read = (self.out_read + 1) % self.out_buf_len;
        }

        // Place grains: grain_phase tracks output sample counter.
        // Each time it passes hop_out, place one grain.
        self.grain_phase += len as f32;

        while self.grain_phase >= hop_out {
            // Check available input data using absolute counters.
            // Grain starts at total_read and extends grain_size samples forward.
            let avail = self.total_written - self.total_read;

            // Need at least a full grain ahead of the read position.
            if avail < self.grain_size as u64 {
                break; // Not enough data yet, wait for next block
            }

            self.grain_phase -= hop_out; // Only subtract after confirming we can place

            // Grain position in the ring buffer
            let grain_pos = (self.total_read % self.buf_len as u64) as usize;

            for j in 0..self.grain_size {
                let src = (grain_pos + j) % self.buf_len;
                let dst = (self.write_out + j) % self.out_buf_len;
                self.out_buf[dst] += self.buf[src] * self.window[j];
            }

            self.total_read += self.hop_in as u64;
            self.write_out = (self.write_out + hop_out_int) % self.out_buf_len;
        }
    }
}

/// Phase Vocoder pitch shifter (HQ mode).
///
/// STFT-based: windowed FFT frames at analysis hops, magnitude interpolation,
/// frequency-shifted phase accumulation, inverse FFT, overlap-add synthesis.
pub struct PhaseVocoderShifter {
    sample_rate: f32,
    fft_size: usize,
    hop: usize,
    window: Vec<f32>,
    window_norm: f32,
    in_buf: Vec<f32>,
    total_written: u64,
    total_read: u64,
    out_buf: Vec<f32>,
    out_buf_len: usize,
    out_read: usize,
    out_write: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    ifft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    acc_phase: Vec<f32>,
}

impl PhaseVocoderShifter {
    pub fn new(sample_rate: f32) -> Self {
        let fft_size = 2048;
        let hop = fft_size / 4;

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let win: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (fft_size - 1) as f32).cos()))
            .collect();
        let win_pow: f32 = win.iter().map(|w| w * w).sum();
        let norm = if win_pow > 0.0 {
            (hop as f32 / win_pow).sqrt()
        } else {
            1.0
        };

        let latency = fft_size;
        let out_buf_len = fft_size * 4;
        // WOLA normalization: hop / (fft_size * sum(w^2)) for perfect reconstruction
        // when using the same Hanning window for both analysis and synthesis.
        let norm_wola = hop as f32 / (fft_size as f32 * win_pow);

        Self {
            sample_rate,
            fft_size,
            hop,
            window: win,
            window_norm: norm_wola,
            in_buf: vec![0.0; fft_size * 8],
            total_written: 0,
            total_read: 0,
            out_buf: vec![0.0; out_buf_len],
            out_buf_len,
            out_read: 0,
            out_write: latency % out_buf_len,
            fft,
            ifft,
            acc_phase: vec![0.0; fft_size / 2 + 1],
        }
    }

    pub fn reset(&mut self, sample_rate: f32) {
        *self = Self::new(sample_rate);
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32], shift_semitones: f32) {
        let len = input.len().min(output.len());

        if shift_semitones.abs() < 0.5 {
            output[..len].copy_from_slice(&input[..len]);
            return;
        }

        let shift = 2.0_f32.powf(shift_semitones / 12.0);
        let fft_size = self.fft_size;
        let hop = self.hop;
        let norm = self.window_norm;
        let in_buf_len = self.in_buf.len();

        for i in 0..len {
            // Write input to ring buffer
            self.in_buf[(self.total_written % in_buf_len as u64) as usize] = input[i];
            self.total_written += 1;

            // Read output
            output[i] = self.out_buf[self.out_read];
            self.out_buf[self.out_read] = 0.0;
            self.out_read = (self.out_read + 1) % self.out_buf_len;
        }

        // Process frames while enough input data is available
        while self.total_written >= self.total_read + fft_size as u64 {
            let frame_offset = (self.total_read % in_buf_len as u64) as usize;

            // --- Analysis: windowed FFT ---
            let mut frame = vec![Complex::new(0.0_f32, 0.0_f32); fft_size];
            for j in 0..fft_size {
                let src = (frame_offset + j) % in_buf_len;
                frame[j] = Complex::new(self.in_buf[src] * self.window[j], 0.0);
            }
            self.fft.process(&mut frame);

            // --- Magnitude ---
            let half = fft_size / 2;
            let mut mag = vec![0.0_f32; half + 1];
            for j in 0..=half {
                mag[j] = frame[j].norm();
            }

            // --- Spectral interpolation of magnitude ---
            let mut mag_shifted = vec![0.0_f32; half + 1];
            for j in 0..=half {
                let src = j as f32 / shift;
                let src_i = src.floor() as usize;
                let frac = src - src_i as f32;
                if src_i < half {
                    let src_n = (src_i + 1).min(half);
                    mag_shifted[j] = mag[src_i] * (1.0 - frac) + mag[src_n] * frac;
                }
            }

            // --- Phase reconstruction ---
            // Each bin's frequency is shifted by `shift`.
            // Phase accumulates by the shifted frequency * hop time.
            let hop_sec = hop as f32 / self.sample_rate;
            for j in 0..=half {
                let bin_freq = j as f32 * self.sample_rate / fft_size as f32;
                let shifted_freq = bin_freq * shift;
                let phase_inc = 2.0 * PI * shifted_freq * hop_sec;
                self.acc_phase[j] = (self.acc_phase[j] + phase_inc) % (2.0 * PI);

                frame[j] = Complex::new(
                    mag_shifted[j] * self.acc_phase[j].cos(),
                    mag_shifted[j] * self.acc_phase[j].sin(),
                );
            }
            // Hermitian symmetry for IFFT
            for j in 1..half {
                frame[fft_size - j] = frame[j].conj();
            }

            // --- Inverse FFT ---
            self.ifft.process(&mut frame);

            // --- Overlap-add into output buffer (with synthesis window) ---
            for j in 0..fft_size {
                let dst = (self.out_write + j) % self.out_buf_len;
                self.out_buf[dst] += frame[j].re * self.window[j] * norm;
            }

            // Advance input read by hop (consume frame)
            self.total_read += hop as u64;
            self.out_write = (self.out_write + hop) % self.out_buf_len;
        }

        // Reset counters if they get too large to prevent overflow
        let min_avail = self.total_written - self.total_read;
        if self.total_written > 1_000_000_000 {
            self.total_written = min_avail;
            self.total_read = 0;
        }
    }
}

pub enum PitchShifter {
    Fast(PsoLaShifter),
    HighQuality(PhaseVocoderShifter),
}

impl PitchShifter {
    pub fn new(sample_rate: f32, mode: ShiftMode) -> Self {
        match mode {
            ShiftMode::PsoLa => PitchShifter::Fast(PsoLaShifter::new(sample_rate)),
            ShiftMode::PhaseVocoder => {
                PitchShifter::HighQuality(PhaseVocoderShifter::new(sample_rate))
            }
        }
    }

    pub fn reset(&mut self, sample_rate: f32, mode: ShiftMode) {
        *self = match mode {
            ShiftMode::PsoLa => PitchShifter::Fast(PsoLaShifter::new(sample_rate)),
            ShiftMode::PhaseVocoder => {
                PitchShifter::HighQuality(PhaseVocoderShifter::new(sample_rate))
            }
        };
    }

    pub fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _pitch_period: f32,
        shift_semitones: f32,
    ) {
        match self {
            PitchShifter::Fast(s) => s.process(input, output, shift_semitones),
            PitchShifter::HighQuality(s) => s.process(input, output, shift_semitones),
        }
    }
}
