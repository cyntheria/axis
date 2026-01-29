use std::str::FromStr;

pub fn pitch_parser(s: &str) -> Result<i32, String> {
    if s.is_empty() {
        return Err("Pitch string cannot be empty".to_string());
    }

    let s = s.trim();
    
    if let Ok(num) = s.parse::<i32>() {
        return Ok(num);
    }

    let mut chars = s.chars().peekable();
    let note_char = chars.next().ok_or("Invalid pitch format")?;
    
    let note = match note_char.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return Err(format!("Invalid note: {}", note_char)),
    };

    let mut semitone_offset = note;
    if let Some(&next) = chars.peek() {
        match next {
            '#' => {
                chars.next();
                semitone_offset += 1;
            }
            'b' | 'B' => {
                chars.next();
                semitone_offset -= 1;
            }
            _ => {}
        }
    }

    let octave_str: String = chars.collect();
    if octave_str.is_empty() {
        return Err("Octave number required".to_string());
    }
    
    let octave: i32 = octave_str.parse()
        .map_err(|_| format!("Invalid octave: {}", octave_str))?;
    
    let midi_note = (octave + 1) * 12 + semitone_offset;
    
    Ok(midi_note)
}

pub fn tempo_parser(s: &str) -> Result<f64, String> {
    let s = s.trim();
    let s = s.strip_prefix('!').unwrap_or(s);
    f64::from_str(s)
        .map_err(|e| format!("Invalid tempo value '{}': {}", s, e))
        .and_then(|v| {
            if v <= 0.0 {
                Err(format!("Tempo must be positive, got {}", v))
            } else {
                Ok(v)
            }
        })
}

pub fn decode_pitchbend(s: &str) -> Vec<f64> {
    let mut result = Vec::new();
    let chunks: Vec<&str> = s.split('#').collect();
    
    for (i, chunk) in chunks.iter().enumerate() {
        if i % 2 == 0 {
            let chars: Vec<char> = chunk.chars().collect();
            for pair in chars.chunks(2) {
                if pair.len() < 2 { break; }
                let mut val = 0i32;
                for &c in pair {
                    let b64_val = match c {
                        'A'..='Z' => c as i32 - 'A' as i32,
                        'a'..='z' => c as i32 - 'a' as i32 + 26,
                        '0'..='9' => c as i32 - '0' as i32 + 52,
                        '+' => 62,
                        '/' => 63,
                        _ => 0,
                    };
                    val = (val << 6) | b64_val;
                }
                if val > 2047 { val -= 4096; }
                result.push(val as f64 / 100.0);
            }
        } else if let Ok(rle) = chunk.parse::<usize>() {
            if let Some(&last) = result.last() {
                for _ in 0..rle {
                    result.push(last);
                }
            }
        }
    }
    
    result
}

pub fn midi_to_hz(midi: f64) -> f64 {
    440.0 * 2.0_f64.powf((midi - 69.0) / 12.0)
}

pub fn hz_to_midi(hz: f64) -> f64 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

pub fn arange(n: i32) -> Vec<f64> {
    (0..n).map(|x| x as f64).collect()
}

pub fn linspace(start: f64, end: f64, num: usize, endpoint: bool) -> Vec<f64> {
    if num == 0 { return Vec::new(); }
    if num == 1 { return vec![start]; }
    let step = if endpoint {
        (end - start) / (num - 1) as f64
    } else {
        (end - start) / num as f64
    };
    (0..num).map(|i| start + i as f64 * step).collect()
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pitch_parser() {
        assert_eq!(pitch_parser("C4"), Ok(60));
        assert_eq!(pitch_parser("C#4"), Ok(61));
        assert_eq!(pitch_parser("Cb4"), Ok(59));
        assert_eq!(pitch_parser("A4"), Ok(69));
        assert_eq!(pitch_parser("B3"), Ok(59));
        assert_eq!(pitch_parser("60"), Ok(60));
    }

    #[test]
    fn test_tempo_parser() {
        assert_eq!(tempo_parser("100"), Ok(100.0));
        assert_eq!(tempo_parser("150.5"), Ok(150.5));
        assert!(tempo_parser("0").is_err());
        assert!(tempo_parser("10").is_ok());
    }

    #[test]
    fn test_decode_pitchbend() {
        assert_eq!(decode_pitchbend("AAAA"), vec![0.0, 0.0]);
        let decoded = decode_pitchbend("AAAA#2#");
        assert_eq!(decoded, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_math_helpers() {
        assert!((midi_to_hz(60.0) - 261.62556).abs() < 0.01);
        assert!((hz_to_midi(440.0) - 69.0).abs() < 0.01);
        assert_eq!(arange(3), vec![0.0, 1.0, 2.0]);
        assert_eq!(linspace(0.0, 1.0, 3, true), vec![0.0, 0.5, 1.0]);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 1.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
    }
}
