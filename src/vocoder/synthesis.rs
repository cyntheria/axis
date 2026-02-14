use rand::{thread_rng, Rng};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;

pub struct StydlEngine {
    pub sample_rate: u32,
    harmonic_phases: Vec<f64>,
    fft_planner: Mutex<FftPlanner<f64>>,
}

impl StydlEngine {
    pub fn new(sample_rate: u32, _fft_size: usize) -> Self {
        let mut rng = thread_rng();
        let phases: Vec<f64> = (0..1024).map(|_| rng.gen::<f64>() * 2.0 * std::f64::consts::PI).collect();
        Self {
            sample_rate,
            harmonic_phases: phases,
            fft_planner: Mutex::new(FftPlanner::new()),
        }
    }

    fn get_amp(spec: &[f64], freq: f64, fs: u32) -> f64 {
        let n = spec.len();
        if n == 0 { return 0.0; }
        let idx_f = freq * (n - 1) as f64 / (fs as f64 / 2.0);
        let i0 = idx_f.floor() as usize;
        if i0 >= n { return 0.0; }
        let i1 = (i0 + 1).min(n - 1);
        let frac = idx_f - i0 as f64;
        let power = spec[i0] * (1.0 - frac) + spec[i1] * frac;
        power.max(0.0).sqrt()
    }

    fn get_bap(bap: &[f64], freq: f64, fs: u32) -> f64 {
        let n = bap.len();
        if n == 0 { return 1.0; }
        let idx_f = freq * (n - 1) as f64 / (fs as f64 / 2.0);
        let i0 = idx_f.floor() as usize;
        if i0 >= n { return 1.0; }
        let i1 = (i0 + 1).min(n - 1);
        let frac = idx_f - i0 as f64;
        let val = bap[i0] * (1.0 - frac) + bap[i1] * frac;
        val.clamp(0.0, 1.0)
    }

    fn synthesize_noise_grain(&self, spec: &[f64], bap: &[f64], fft_size: usize) -> Vec<f64> {
        let mut planner = self.fft_planner.lock().unwrap();
        let fft = planner.plan_fft_inverse(fft_size);
        let mut rng = thread_rng();

        let mut buffer = vec![Complex::new(0.0, 0.0); fft_size];
        let num_bins = fft_size / 2 + 1;

        for k in 0..num_bins {
            let freq = k as f64 * self.sample_rate as f64 / fft_size as f64;
            let amp = Self::get_amp(spec, freq, self.sample_rate);
            let bap_val = Self::get_bap(bap, freq, self.sample_rate);

            let target_amp = amp * bap_val;
            let phase = rng.gen::<f64>() * 2.0 * std::f64::consts::PI;

            let val = Complex::from_polar(target_amp, phase);
            buffer[k] = val;
            if k > 0 && k < num_bins - 1 {
                buffer[fft_size - k] = val.conj();
            }
        }

        fft.process(&mut buffer);
        buffer.iter().map(|c| c.re / fft_size as f64).collect()
    }

    pub fn synthesize(&mut self, f0: &[f64], spectral: &[Vec<f64>], aperiodicity: &[Vec<f64>]) -> Vec<f64> {
        let hop_size = 256;
        let num_frames = f0.len();
        let total_samples = num_frames * hop_size;
        let mut output = vec![0.0; total_samples + 2048];

        // ── Sinusoidal & Noise Dual-Stream ──
        for f_idx in 0..num_frames.saturating_sub(1) {
            let f0_start = f0[f_idx];
            let f0_end = f0[f_idx + 1];

            let out_start = f_idx * hop_size;
            
            // Generate a high-resolution noise stream for this frame (Overlap-Add)
            let noise_fft_size = 1024;
            let noise_grain = self.synthesize_noise_grain(&spectral[f_idx], &aperiodicity[f_idx], noise_fft_size);

            for t in 0..hop_size {
                let out_idx = out_start + t;
                if out_idx >= total_samples { break; }

                let alpha = t as f64 / hop_size as f64;
                let current_f0 = f0_start * (1.0 - alpha) + f0_end * alpha;
                
                // Voice activity detector with smoothing (to prevent snaps)
                // If F0 is near zero, we fade out the sines.
                let voicing_v0 = if f0_start > 40.0 { 1.0 } else { 0.0 };
                let voicing_v1 = if f0_end > 40.0 { 1.0 } else { 0.0 };
                let voicing_weight = voicing_v0 * (1.0 - alpha) + voicing_v1 * alpha;

                // 1. Voiced Stream (Sinusoidal)
                let mut sample_voiced = 0.0;
                if voicing_weight > 0.001 && current_f0 > 40.0 {
                    let num_harmonics = (self.sample_rate as f64 / (2.0 * current_f0)).floor() as usize;
                    let num_harmonics = num_harmonics.min(512);

                    for k in 1..=num_harmonics {
                        let phase_inc = 2.0 * std::f64::consts::PI * (current_f0 * k as f64) / self.sample_rate as f64;
                        self.harmonic_phases[k % 1024] = (self.harmonic_phases[k % 1024] + phase_inc) % (2.0 * std::f64::consts::PI);

                        let freq = current_f0 * k as f64;
                        let amp_s = Self::get_amp(&spectral[f_idx], freq, self.sample_rate);
                        let amp_e = Self::get_amp(&spectral[f_idx + 1], freq, self.sample_rate);
                        let amp = amp_s * (1.0 - alpha) + amp_e * alpha;

                        let bap_s = Self::get_bap(&aperiodicity[f_idx], freq, self.sample_rate);
                        let bap_e = Self::get_bap(&aperiodicity[f_idx + 1], freq, self.sample_rate);
                        let bap = bap_s * (1.0 - alpha) + bap_e * alpha;

                        // Voiced component is purely the NON-aperiodic part
                        let v_comp = (1.0 - bap).max(0.0);
                        sample_voiced += amp * v_comp * self.harmonic_phases[k % 1024].sin();
                    }
                } else if current_f0 > 40.0 {
                    // Even if suppressed, we must keep phase accumulators moving to maintain coherence
                    let num_harmonics = (self.sample_rate as f64 / (2.0 * current_f0)).floor() as usize;
                    let num_harmonics = num_harmonics.min(512);
                    for k in 1..=num_harmonics {
                        let phase_inc = 2.0 * std::f64::consts::PI * (current_f0 * k as f64) / self.sample_rate as f64;
                        self.harmonic_phases[k % 1024] = (self.harmonic_phases[k % 1024] + phase_inc) % (2.0 * std::f64::consts::PI);
                    }
                }

                // Balanced gain scaling for voiced stream
                let num_v_h = (self.sample_rate as f64 / (2.0 * current_f0.max(40.0))).floor().max(1.0);
                let voiced_signal = sample_voiced * voicing_weight * (0.2 / num_v_h.powf(0.5));

                // 2. Unvoiced Stream (Noise Grain from OLA)
                let noise_win = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * t as f64 / (noise_fft_size as f64 - 1.0)).cos());
                let unvoiced_signal = noise_grain[t % noise_grain.len()] * noise_win;

                output[out_idx] += voiced_signal + unvoiced_signal;
            }
        }

        // ── Peak Normalization ──
        let peak = output.iter().take(total_samples).map(|x| x.abs()).fold(0.0_f64, f64::max);
        if peak > 0.001 {
            let scale = 0.85 / peak;
            for x in output[..total_samples].iter_mut() {
                *x *= scale;
            }
        }

        output.truncate(total_samples);
        output
    }
}