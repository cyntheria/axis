use rand::{thread_rng, Rng};

pub struct StydlEngine {
    pub sample_rate: u32,
    fundamental_phase: f64,
}

impl StydlEngine {
    pub fn new(sample_rate: u32, _fft_size: usize) -> Self {
        Self { 
            sample_rate, 
            fundamental_phase: 0.0,
        }
    }

    fn get_amp(spec: &[f64], ap: Option<&Vec<f64>>, freq: f64, fs: u32) -> (f64, f64) {
        let feat_idx = freq * (spec.len() - 1) as f64 / (fs as f64 / 2.0);
        let idx_low = feat_idx.floor() as usize;
        let idx_high = (idx_low + 1).min(spec.len() - 1);
        let frac = feat_idx - idx_low as f64;
        
        let power = spec[idx_low] * (1.0 - frac) + spec[idx_high] * frac;
        
        let bap_val = if let Some(a) = ap {
            let ap_idx = (freq * (a.len() - 1) as f64 / (fs as f64 / 2.0)) as usize;
            a[ap_idx.min(a.len() - 1)]
        } else {
            0.1
        };

        // Simple magnitude from power
        let mag = power.max(0.0).sqrt();
        let voiced_amp = mag * (1.0 - bap_val).sqrt();
        let aperiodic_amp = mag * bap_val.sqrt();
        
        (voiced_amp, aperiodic_amp)
    }

    pub fn synthesize(
        &mut self, 
        f0: &[f64], 
        spectral: &[Vec<f64>], 
        aperiodicity: &[Vec<f64>],
        source: &[f64]
    ) -> Vec<f64> {
        let hop_size = 256; 
        let mut output = vec![0.0; f0.len() * hop_size + 1024];
        let mut rng = thread_rng();

        for frame_idx in 0..f0.len().saturating_sub(1) {
            let n0 = frame_idx * hop_size;
            let f0_0 = f0[frame_idx];
            let f0_1 = f0[frame_idx + 1];
            
            let spec0 = &spectral[frame_idx];
            let spec1 = &spectral[frame_idx + 1];
            
            let ap0 = aperiodicity.get(frame_idx);
            let ap1 = aperiodicity.get(frame_idx + 1);

            if f0_0 > 40.0 && f0_1 > 40.0 {
                let num_harmonics = ((self.sample_rate as f64 / 2.0) / f0_0.max(f0_1)) as usize;
                let k_limit = num_harmonics.min(512);

                for i in 0..hop_size {
                    let t = n0 + i;
                    let alpha = i as f64 / hop_size as f64;
                    
                    let target_f0 = f0_0 * (1.0 - alpha) + f0_1 * alpha;
                    let phase_inc = 2.0 * std::f64::consts::PI * target_f0 / self.sample_rate as f64;
                    self.fundamental_phase = (self.fundamental_phase + phase_inc) % (2.0 * std::f64::consts::PI);
                    
                    let mut sample_out = 0.0;
                    for k in 1..=k_limit {
                        let freq = k as f64 * target_f0;
                        if freq >= self.sample_rate as f64 / 2.0 { break; }

                        let (v0, a0) = Self::get_amp(spec0, ap0, freq, self.sample_rate);
                        let (v1, a1) = Self::get_amp(spec1, ap1, freq, self.sample_rate);
                        let v_amp = v0 * (1.0 - alpha) + v1 * alpha;
                        let a_amp = a0 * (1.0 - alpha) + a1 * alpha;

                        // Simple phase: just multiply fundamental phase by harmonic number
                        let phase_k = self.fundamental_phase * k as f64;
                        
                        let voiced = v_amp * phase_k.sin();
                        
                        // Noise only in high frequencies
                        let aperiodic = if freq > 2000.0 {
                            a_amp * (rng.gen::<f64>() * 2.0 - 1.0)
                        } else {
                            0.0
                        };
                        
                        sample_out += voiced + aperiodic;
                    }
                    // Much lower gain - just 1.0 to start
                    output[t] += sample_out;
                }
            }
            
            // Unvoiced segments use source
            if !source.is_empty() {
                let is_unvoiced = f0_0 < 40.0;
                if is_unvoiced {
                    let multiplier = 0.5;
                    let src_start = (frame_idx * 256).min(source.len().saturating_sub(hop_size * 2));
                    for i in 0..hop_size * 2 {
                        let t = n0 + i;
                        if t < output.len() {
                            let window = 0.5 * (1.0 - (std::f64::consts::PI * i as f64 / (hop_size as f64 * 2.0)).cos());
                            output[t] += source[src_start + i] * multiplier * window;
                        }
                    }
                }
            }
        }
        output
    }
}
