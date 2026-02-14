use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Mutex;

pub struct TrickResolve {
    sample_rate: u32,
    planner: Mutex<FftPlanner<f64>>,
}

impl TrickResolve {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            planner: Mutex::new(FftPlanner::new()),
        }
    }

    pub fn resolve(&self, input: &[f64], f0: f64, fft_size: usize) -> Vec<f64> {
        if f0 <= 40.0 {
            let mut planner = self.planner.lock().unwrap();
            let fft = planner.plan_fft_forward(fft_size);
            let mut complex_input: Vec<Complex<f64>> = input.iter().take(fft_size).map(|&x| Complex::new(x, 0.0)).collect();
            if complex_input.len() < fft_size {
                complex_input.resize(fft_size, Complex::new(0.0, 0.0));
            }
            fft.process(&mut complex_input);
            return complex_input.iter().take(fft_size / 2 + 1).map(|c| c.norm_sqr() / (fft_size as f64)).collect();
        }

        let window_len = (3.0 * self.sample_rate as f64 / f0) as usize;
        let mut window_sum = 0.0;
        let mut windowed = vec![0.0; fft_size];
        for i in 0..window_len.min(input.len()).min(fft_size) {
            let pos = (i as f64 + 0.5) / window_len as f64;
            let win = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * pos).cos());
            windowed[i] = input[i] * win;
            window_sum += win;
        }

        let mut planner = self.planner.lock().unwrap();
        let fft = planner.plan_fft_forward(fft_size);
        let mut buffer: Vec<Complex<f64>> = windowed.iter().map(|&x| Complex::new(x, 0.0)).collect();
        fft.process(&mut buffer);

        // Normalize by window_sum (linear) to boost energy as suggested
        let power_spec: Vec<f64> = buffer.iter().take(fft_size / 2 + 1)
            .map(|c| c.norm_sqr() / window_sum).collect();

        let smoothing_width = (f0 * fft_size as f64 / self.sample_rate as f64) as usize;
        let mut smoothed = vec![0.0; power_spec.len()];
        if smoothing_width > 1 {
            let mut sum = 0.0;
            for i in 0..smoothing_width.min(power_spec.len()) {
                sum += power_spec[i];
            }
            for i in 0..power_spec.len() {
                // Return mean power in the smoothing band (linear normalization)
                smoothed[i] = sum / smoothing_width as f64; 
                if i + smoothing_width / 2 < power_spec.len() {
                    sum += power_spec[i + smoothing_width / 2];
                }
                if i >= smoothing_width / 2 {
                    sum -= power_spec[i - smoothing_width / 2];
                }
            }
        } else {
            smoothed = power_spec;
        }

        smoothed
    }
}
