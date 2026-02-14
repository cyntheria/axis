use biquad::{Biquad, Coefficients, ToHertz, Type, DirectForm1};
use anyhow::{anyhow, Result};

pub fn forward_backward_filter<F: Biquad<f64>>(signal: &mut [f64], filter: &mut F) {
    signal.iter_mut().for_each(|x| *x = filter.run(*x));
    filter.reset_state();
    signal.reverse();
    signal.iter_mut().for_each(|x| *x = filter.run(*x));
    filter.reset_state();
    signal.reverse();
}

pub fn make_coefficients(f_type: Type<f64>, fs: f64, freq: f64, q: f64) -> Result<Coefficients<f64>> {
    Coefficients::<f64>::from_params(f_type, fs.hz(), freq.hz(), q).map_err(|_| anyhow!("Failed to create filter coefficients"))
}

pub fn apply_vocal_enhancement(samples: &mut [f64], sample_rate: u32) -> Result<()> {
    let fs = sample_rate as f64;
    
    // Simple HPF to clean up low end rumble, otherwise keep it pure
    let hpf_coeffs = make_coefficients(Type::HighPass, fs, 60.0, 0.707)?;
    let mut hpf = DirectForm1::<f64>::new(hpf_coeffs);
    forward_backward_filter(samples, &mut hpf);

    Ok(())
}
