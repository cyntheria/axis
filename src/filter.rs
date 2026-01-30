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
    
    let hpf_coeffs = make_coefficients(Type::HighPass, fs, 80.0, 0.707)?;
    let mut hpf = DirectForm1::<f64>::new(hpf_coeffs);
    forward_backward_filter(samples, &mut hpf);

    let peak_coeffs = make_coefficients(Type::PeakingEQ(2.5), fs, 3500.0, 1.0)?;
    let mut peak = DirectForm1::<f64>::new(peak_coeffs);
    forward_backward_filter(samples, &mut peak);

    let air_coeffs = make_coefficients(Type::HighShelf(1.5), fs, 12000.0, 0.707)?;
    let mut air = DirectForm1::<f64>::new(air_coeffs);
    forward_backward_filter(samples, &mut air);

    for x in samples.iter_mut() {
        let val = *x;
        if val.abs() > 0.001 {
            *x = val.signum() * (1.0 - (-val.abs() * 1.5).exp());
        }
    }

    Ok(())
}
