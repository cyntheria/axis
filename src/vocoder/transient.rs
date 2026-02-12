pub struct TransientDetector {
    pub window_size: usize,
    pub step: usize,
}

impl TransientDetector {
    pub fn new(window_size: usize, step: usize) -> Self {
        Self { window_size, step }
    }

    pub fn detect(&self, input: &[f64]) -> Vec<bool> {
        let mut results = Vec::new();
        let mut last_energy = 0.0;
        for i in (0..input.len()).step_by(self.step) {
            let end = (i + self.window_size).min(input.len());
            let chunk = &input[i..end];
            let energy: f64 = chunk.iter().map(|&x| x * x).sum();
            if energy > last_energy * 3.0 && energy > 0.01 {
                results.push(true);
            } else {
                results.push(false);
            }
            last_energy = energy;
        }
        results
    }
}
