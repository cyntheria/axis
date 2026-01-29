pub struct Flags {
    pub gender: f64,
    pub breathiness: f64,
}

impl std::str::FromStr for Flags {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut gender = 0.0;
        let mut breathiness = 50.0;

        let s = s.replace("/", "");
        
        let mut i = 0;
        let chars: Vec<char> = s.chars().collect();
        while i < chars.len() {
            match chars[i] {
                'g' | 'G' => {
                    let mut start = i + 1;
                    while start < chars.len() && (chars[start].is_numeric() || chars[start] == '-' || chars[start] == '.') {
                        start += 1;
                    }
                    if let Ok(val) = s[i+1..start].parse::<f64>() {
                        gender = val;
                    }
                    i = start;
                }
                'B' | 'b' => {
                    let mut start = i + 1;
                    while start < chars.len() && (chars[start].is_numeric() || chars[start] == '-' || chars[start] == '.') {
                        start += 1;
                    }
                    if let Ok(val) = s[i+1..start].parse::<f64>() {
                        breathiness = val;
                    }
                    i = start;
                }
                _ => i += 1,
            }
        }

        Ok(Flags { gender, breathiness })
    }
}
