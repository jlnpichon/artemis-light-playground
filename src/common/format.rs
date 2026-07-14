use alloy::primitives::U256;

/// Format a [`U256`] as a human-readable string with SI-like suffixes.
///
/// Examples: `1_000` -> `1.00K`, `1_000_000` -> `1.00M`, `1_000_000_000` -> `1.00B`.
pub fn format_si(value: U256) -> String {
    if value.is_zero() {
        return "0".into();
    }
    let s = value.to_string();
    let len = s.len();

    const SUFFIXES: &[&str] = &[
        "", "K", "M", "B", "T", "Qa", "Qi", "Sx", "Sp", "Oc", "No", "Dc",
    ];
    let group = (len - 1) / 3;
    let idx = group.min(SUFFIXES.len() - 1);

    if idx == 0 {
        return s;
    }

    let sig_len = len - idx * 3;
    let int_part = &s[..sig_len];
    let frac_end = (sig_len + 2).min(len);
    let frac = &s[sig_len..frac_end];

    if frac.is_empty() {
        format!("{int_part}{}", SUFFIXES[idx])
    } else {
        format!("{int_part}.{frac}{}", SUFFIXES[idx])
    }
}
