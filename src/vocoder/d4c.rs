use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;

pub struct D4C {
    sample_rate: u32,
    planner: Mutex<FftPlanner<f64>>,
}

impl D4C {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            planner: Mutex::new(FftPlanner::new()),
        }
    }

    pub fn estimate(&self, input: &[f64], f0: f64, fft_size: usize) -> Vec<f64> {
        let num_bins = fft_size / 2 + 1;
        if f0 <= 40.0 {
            return vec![1.0; num_bins];
        }

        let mut baps = vec![0.0; num_bins];
        let mut planner = self.planner.lock().unwrap();
        let fft = planner.plan_fft_forward(fft_size);
        
        let mut complex_input: Vec<Complex<f64>> = input.iter().take(fft_size).map(|&x| Complex::new(x, 0.0)).collect();
        complex_input.resize(fft_size, Complex::new(0.0, 0.0));
        fft.process(&mut complex_input);

        for bin in 0..num_bins {
            let freq = bin as f64 * self.sample_rate as f64 / fft_size as f64;
            if freq < 2000.0 {
                baps[bin] = 0.005;
            } else if freq < 5000.0 {
                baps[bin] = 0.02;
            } else {
                baps[bin] = 0.08;
            }
        }

        baps
    }
}
