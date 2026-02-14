use crate::vocoder::transient::TransientDetector;
use crate::vocoder::synthesis::StydlEngine;
use crate::vocoder::dio::Dio;
use crate::vocoder::trickresolve::TrickResolve;
use crate::vocoder::d4c::D4C;

pub struct StydlVocoder {
    pub sample_rate: u32,
    pub fft_size: usize,
    pub detector: TransientDetector,
    pub engine: StydlEngine,
    pub f0_estimator: Dio,
    pub spectral_resolver: TrickResolve,
    pub aperiodicity_estimator: D4C,
}

impl StydlVocoder {
    pub fn new(sample_rate: u32, _fft_size: usize) -> Self {
        let max_fft_size = 4096;
        Self { 
            sample_rate,
            fft_size: max_fft_size,
            detector: TransientDetector::new(512, 256),
            engine: StydlEngine::new(sample_rate, max_fft_size),
            f0_estimator: Dio::new(sample_rate),
            spectral_resolver: TrickResolve::new(sample_rate),
            aperiodicity_estimator: D4C::new(sample_rate),
        }
    }

    pub fn process(&mut self, f0: &[f64], spectral: &[Vec<f64>], aperiodicity: &[Vec<f64>], source: &[f64], _source_frames: &[f64]) -> Vec<f64> {
        let mut refined_spectral = Vec::with_capacity(f0.len());
        let mut refined_aperiodicity = Vec::with_capacity(f0.len());

        let hop_size = 256;
        for (i, &f) in f0.iter().enumerate() {
            let start = (i * hop_size).min(source.len());
            let end = (start + self.fft_size).min(source.len());
            let chunk = &source[start..end];
            
            if chunk.is_empty() || f < 40.0 {
                refined_spectral.push(spectral[i].clone());
                refined_aperiodicity.push(aperiodicity[i].clone());
                continue;
            }

            refined_spectral.push(spectral[i].clone());
            refined_aperiodicity.push(aperiodicity[i].clone());
        }

        self.engine.synthesize(f0, &refined_spectral, &refined_aperiodicity)
    }
}
