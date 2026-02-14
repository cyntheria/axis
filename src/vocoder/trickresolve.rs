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
            let mut windowed = vec![0.0; fft_size];
            let mut window_sum = 0.0;
            for i in 0..input.len().min(fft_size) {
                let win = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * (i as f64 + 0.5) / fft_size as f64).cos());
                windowed[i] = input[i] * win;
                window_sum += win;
            }
            let mut complex_input: Vec<Complex<f64>> = windowed.iter().map(|&x| Complex::new(x, 0.0)).collect();
            fft.process(&mut complex_input);
            return complex_input.iter().take(fft_size / 2 + 1)
                .map(|c| (c.norm_sqr() * 4.0) / (window_sum * window_sum)).collect();
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

        // Correct normalization to preserve peak amplitude squared
        let power_spec: Vec<f64> = buffer.iter().take(fft_size / 2 + 1)
            .map(|c| (c.norm_sqr() * 4.0) / (window_sum * window_sum)).collect();

        // Refined smoothing width: frequency-dependent to suppress high-frequency imaging
        // At low frequencies, we stay tight to the harmonic spacing.
        // At high frequencies, we broaden the window to ensure a soft envelope for noise.
        let mut smoothed = vec![0.0; power_spec.len()];
        
        for i in 0..power_spec.len() {
            let freq = i as f64 * self.sample_rate as f64 / fft_size as f64;
            let base_width = (f0 * fft_size as f64 / self.sample_rate as f64).round() as usize;
            
            // Gradually increase smoothing width as frequency goes up
            let width_scale = 1.0 + (freq / 5000.0).powi(2);
            let width = (base_width as f64 * width_scale).round() as usize;
            let width = width.max(2);
            
            let half = width / 2;
            let start = i.saturating_sub(half);
            let end = (i + half + 1).min(power_spec.len());
            
            let mut current_sum = 0.0;
            for j in start..end {
                current_sum += power_spec[j];
            }
            smoothed[i] = current_sum / (end - start) as f64;
        }

        smoothed
    }
}
