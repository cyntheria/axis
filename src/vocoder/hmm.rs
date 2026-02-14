use log::debug;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoicingState {
    Voiced,
    Unvoiced,
}

pub struct VoicingHmm {
    // Transition probabilities (log domain)
    log_p_vv: f64, // P(voiced -> voiced)
    log_p_vu: f64, // P(voiced -> unvoiced)
    log_p_uv: f64, // P(unvoiced -> voiced)
    log_p_uu: f64, // P(unvoiced -> unvoiced)
    f0_threshold: f64,
}

impl VoicingHmm {
    pub fn new() -> Self {
        // Speech is mostly voiced, with occasional unvoiced transitions
        let p_vv: f64 = 0.95;
        let p_vu: f64 = 1.0 - p_vv;
        let p_uu: f64 = 0.85;
        let p_uv: f64 = 1.0 - p_uu;

        Self {
            log_p_vv: p_vv.ln(),
            log_p_vu: p_vu.ln(),
            log_p_uv: p_uv.ln(),
            log_p_uu: p_uu.ln(),
            f0_threshold: 40.0,
        }
    }

    fn emission_log_prob(&self, f0: f64, state: VoicingState) -> f64 {
        match state {
            VoicingState::Voiced => {
                if f0 >= self.f0_threshold {
                    // Observing F0 is consistent with voiced state
                    -0.5
                } else {
                    // Strong penalty for missing F0 in voiced state
                    -15.0
                }
            }
            VoicingState::Unvoiced => {
                if f0 < self.f0_threshold {
                    // Lack of F0 is consistent with unvoiced state
                    -0.05
                } else {
                    // High penalty for finding F0 in unvoiced state
                    // This prevents "humming" on breathy consonants
                    -8.0
                }
            }
        }
    }

    /// Viterbi decoding: find the most likely sequence of V/UV states
    pub fn decode(&self, f0_raw: &[f64]) -> Vec<VoicingState> {
        let n = f0_raw.len();
        if n == 0 { return vec![]; }

        let states = [VoicingState::Voiced, VoicingState::Unvoiced];

        // Viterbi tables
        let mut viterbi = vec![[f64::NEG_INFINITY; 2]; n];
        let mut backptr = vec![[0usize; 2]; n];

        // Initial probabilities
        let init_voiced = if f0_raw[0] >= self.f0_threshold { -0.3 } else { -2.0 };
        let init_unvoiced = if f0_raw[0] < self.f0_threshold { -0.3 } else { -2.0 };

        viterbi[0][0] = init_voiced + self.emission_log_prob(f0_raw[0], VoicingState::Voiced);
        viterbi[0][1] = init_unvoiced + self.emission_log_prob(f0_raw[0], VoicingState::Unvoiced);

        // Forward pass
        for t in 1..n {
            for (j, &cur_state) in states.iter().enumerate() {
                let emit = self.emission_log_prob(f0_raw[t], cur_state);
                let mut best_score = f64::NEG_INFINITY;
                let mut best_prev = 0;

                for (i, &_prev_state) in states.iter().enumerate() {
                    let trans = match (i, j) {
                        (0, 0) => self.log_p_vv,
                        (0, 1) => self.log_p_vu,
                        (1, 0) => self.log_p_uv,
                        (1, 1) => self.log_p_uu,
                        _ => unreachable!(),
                    };
                    let score = viterbi[t - 1][i] + trans;
                    if score > best_score {
                        best_score = score;
                        best_prev = i;
                    }
                }

                viterbi[t][j] = best_score + emit;
                backptr[t][j] = best_prev;
            }
        }

        // Backtrace
        let mut path = vec![VoicingState::Unvoiced; n];
        let last_state = if viterbi[n - 1][0] > viterbi[n - 1][1] { 0 } else { 1 };
        path[n - 1] = states[last_state];

        let mut cur = last_state;
        for t in (0..n - 1).rev() {
            cur = backptr[t + 1][cur];
            path[t] = states[cur];
        }

        let voiced_count = path.iter().filter(|s| **s == VoicingState::Voiced).count();
        debug!("HMM V/UV: {}/{} frames voiced", voiced_count, n);

        path
    }

    /// Smooth F0 using HMM V/UV decisions and median filtering
    pub fn smooth_f0(&self, f0_raw: &[f64]) -> Vec<f64> {
        let voicing = self.decode(f0_raw);
        let n = f0_raw.len();
        let mut f0_smooth = vec![0.0; n];

        // Apply V/UV decision: zero out F0 for unvoiced frames
        for i in 0..n {
            f0_smooth[i] = match voicing[i] {
                VoicingState::Voiced => {
                    if f0_raw[i] >= self.f0_threshold {
                        f0_raw[i]
                    } else {
                        // Interpolate from neighbors if HMM says voiced but detector missed
                        let prev = (0..i).rev().find(|&j| f0_raw[j] >= self.f0_threshold);
                        let next = (i + 1..n).find(|&j| f0_raw[j] >= self.f0_threshold);
                        match (prev, next) {
                            (Some(p), Some(nx)) => {
                                let alpha = (i - p) as f64 / (nx - p) as f64;
                                f0_raw[p] * (1.0 - alpha) + f0_raw[nx] * alpha
                            }
                            (Some(p), None) => f0_raw[p],
                            (None, Some(nx)) => f0_raw[nx],
                            (None, None) => 0.0,
                        }
                    }
                }
                VoicingState::Unvoiced => 0.0,
            };
        }

        // Median filter on voiced segments to remove pitch spikes
        let median_radius = 2;
        let mut f0_median = f0_smooth.clone();
        for i in 0..n {
            if f0_smooth[i] > 0.0 {
                let start = i.saturating_sub(median_radius);
                let end = (i + median_radius + 1).min(n);
                let mut window: Vec<f64> = f0_smooth[start..end]
                    .iter()
                    .filter(|&&v| v > 0.0)
                    .cloned()
                    .collect();
                if !window.is_empty() {
                    window.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    f0_median[i] = window[window.len() / 2];
                }
            }
        }

        f0_median
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voiced_sequence() {
        let hmm = VoicingHmm::new();
        let f0 = vec![200.0, 205.0, 198.0, 0.0, 210.0, 200.0];
        let smoothed = hmm.smooth_f0(&f0);
        // The gap at index 3 should be interpolated since neighbors are voiced
        assert!(smoothed[3] > 0.0, "HMM should interpolate voiced gap");
    }

    #[test]
    fn test_unvoiced_sequence() {
        let hmm = VoicingHmm::new();
        let f0 = vec![0.0, 0.0, 0.0, 0.0, 0.0];
        let smoothed = hmm.smooth_f0(&f0);
        assert!(smoothed.iter().all(|&v| v == 0.0), "All unvoiced should stay zero");
    }

    #[test]
    fn test_spike_removal() {
        let hmm = VoicingHmm::new();
        let f0 = vec![200.0, 200.0, 800.0, 200.0, 200.0];
        let smoothed = hmm.smooth_f0(&f0);
        // The spike at index 2 should be smoothed by median filter
        assert!((smoothed[2] - 200.0).abs() < 50.0, "Spike should be smoothed: got {}", smoothed[2]);
    }
}
