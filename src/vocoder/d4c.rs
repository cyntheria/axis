use rand::{thread_rng, Rng};
use std::sync::Mutex;

pub struct D4C {
    sample_rate: u32,
}

impl D4C {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
        }
    }

    pub fn estimate(&self, _input: &[f64], f0: f64, fft_size: usize) -> Vec<f64> {
        let num_bins = fft_size / 2 + 1;
        if f0 <= 40.0 {
            return vec![1.0; num_bins];
        }

        let mut baps = vec![0.0; num_bins];
        
        let mut rng = rand::thread_rng();
        for bin in 0..num_bins {
            let freq = bin as f64 * self.sample_rate as f64 / fft_size as f64;
            // Base aperiodicity
            let base = if freq < 1000.0 {
                0.01
            } else if freq < 3000.0 {
                0.05
            } else if freq < 6000.0 {
                0.15
            } else {
                0.35 + (freq - 6000.0) / (self.sample_rate as f64 / 2.0 - 6000.0) * 0.45
            };

            // Add temporal micro-jitter to prevent "frozen" texture
            let jitter = (rng.gen::<f64>() - 0.5) * 0.02;
            baps[bin] = (base + jitter).clamp(0.0, 1.0);
        }

        baps
    }
}
