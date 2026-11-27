const TWO_DECIMALS_BELOW: f64 = 10.0;
const ONE_DECIMAL_BELOW: f64 = 99.95;
const THOUSANDS_GROUP: usize = 3;

pub fn fmt_aic(v: f64) -> String {
    let a = v.abs();
    let body = if a == 0.0 {
        "0".to_string()
    } else if a < TWO_DECIMALS_BELOW {
        format!("{a:.2}")
    } else if a < ONE_DECIMAL_BELOW {
        format!("{a:.1}")
    } else {
        group_thousands(&format!("{a:.0}"))
    };
    if v < 0.0 { format!("-{body}") } else { body }
}

pub fn fmt_days(days: i64) -> String {
    if days == 1 { "1 day".into() } else { format!("{days} days") }
}

fn group_thousands(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / THOUSANDS_GROUP);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % THOUSANDS_GROUP == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aic_precision_follows_the_size_of_the_value() {
        assert_eq!(fmt_aic(0.0), "0");
        assert_eq!(fmt_aic(1.5), "1.50");
        assert_eq!(fmt_aic(9.5), "9.50");
        assert_eq!(fmt_aic(12.26), "12.3");
        assert_eq!(fmt_aic(99.94), "99.9");
    }

    #[test]
    fn aic_that_rounds_to_one_hundred_reads_as_one_hundred() {
        assert_eq!(fmt_aic(99.96), "100");
        assert_eq!(fmt_aic(100.0), "100");
    }

    #[test]
    fn aic_groups_thousands_and_keeps_the_sign() {
        assert_eq!(fmt_aic(999.4), "999");
        assert_eq!(fmt_aic(12250.0), "12,250");
        assert_eq!(fmt_aic(1234567.0), "1,234,567");
        assert_eq!(fmt_aic(-2500.0), "-2,500");
    }

    #[test]
    fn day_counts_pluralise() {
        assert_eq!(fmt_days(0), "0 days");
        assert_eq!(fmt_days(1), "1 day");
        assert_eq!(fmt_days(2), "2 days");
    }
}
