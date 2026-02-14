pub struct Dio {
    pub sample_rate: u32,
}

impl Dio {
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    pub fn estimate(&self, input: &[f64]) -> Vec<f64> {
        let hop_size = 256;
        let num_frames = input.len() / hop_size;
        let mut f0 = vec![0.0; num_frames];
        for i in 0..num_frames {
            let start = i * hop_size;
            let end = (start + 1024).min(input.len());
            let chunk = &input[start..end];
            f0[i] = self.detect_pitch(chunk);
        }
        self.stonemask(input, &f0)
    }

    fn detect_pitch(&self, chunk: &[f64]) -> f64 {
        if chunk.len() < 512 { return 0.0; }
        let mut max_corr = 0.0;
        let mut best_lag = 0;
        let min_lag = self.sample_rate as usize / 500;
        let max_lag = self.sample_rate as usize / 50;
        for lag in min_lag..max_lag {
            if lag >= chunk.len() { break; }
            let mut corr = 0.0;
            for i in 0..chunk.len().saturating_sub(lag) {
                corr += chunk[i] * chunk[i + lag];
            }
            if corr > max_corr {
                max_corr = corr;
                best_lag = lag;
            }
        }
        if best_lag > 0 {
            self.sample_rate as f64 / best_lag as f64
        } else {
            0.0
        }
    }

    fn stonemask(&self, input: &[f64], f0: &[f64]) -> Vec<f64> {
        let mut refined_f0 = f0.to_vec();
        let hop_size = 256;
        for (i, &initial_f0) in f0.iter().enumerate() {
            if initial_f0 <= 40.0 { continue; }
            let start = i * hop_size;
            let end = (start + 1024).min(input.len());
            let chunk = &input[start..end];
            refined_f0[i] = self.refine_local(chunk, initial_f0);
        }
        refined_f0
    }

    fn refine_local(&self, chunk: &[f64], initial_f0: f64) -> f64 {
        if chunk.len() < 2 { return initial_f0; }
        let mut best_f0 = initial_f0;
        let mut max_energy = 0.0;
        for offset in -2..=2 {
            let test_f0 = initial_f0 + offset as f64 * 0.5;
            let energy = self.calculate_harmonic_energy(chunk, test_f0);
            if energy > max_energy {
                max_energy = energy;
                best_f0 = test_f0;
            }
        }
        best_f0
    }

    fn calculate_harmonic_energy(&self, chunk: &[f64], f0: f64) -> f64 {
        let mut energy = 0.0;
        let period = self.sample_rate as f64 / f0;
        for i in 0..chunk.len() {
            let weight = (1.0 + (2.0 * std::f64::consts::PI * i as f64 / period).cos()) / 2.0;
            energy += chunk[i] * chunk[i] * weight;
        }
        energy
    }
}
