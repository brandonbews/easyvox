//! Word error rate over normalized text.
//!
//! Normalization: lowercase; "$N" -> "N dollars"; "N%" -> "N percent"; digit-group commas
//! removed ("1,250" -> "1250"); hyphens/slashes/colons split words; other punctuation and
//! apostrophes dropped; small number words (zero..twenty, tens) -> digits; "dr"/"mr"/"mrs"
//! expanded. Anything else that differs (e.g. "three hundred" vs "300") counts as an error.

pub fn normalize(s: &str) -> Vec<String> {
    let mut t = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let next_digit = chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
        match c {
            ',' | '.' if prev_digit && next_digit => {
                if c == '.' {
                    t.push('.');
                }
            }
            '$' => t.push_str(" $"),
            '%' => t.push_str(" percent "),
            '-' | '/' | ':' | '\u{2014}' | '\u{2013}' => t.push(' '),
            c if c.is_alphanumeric() || c.is_whitespace() => t.extend(c.to_lowercase()),
            _ => {}
        }
    }
    let mut out: Vec<String> = Vec::new();
    let mut dollars_pending = false;
    for w in t.split_whitespace() {
        if let Some(rest) = w.strip_prefix('$') {
            if !rest.is_empty() {
                out.push(rest.to_string());
                out.push("dollars".into());
            } else {
                dollars_pending = true;
            }
            continue;
        }
        let w = match w {
            "zero" => "0",
            "one" => "1",
            "two" => "2",
            "three" => "3",
            "four" => "4",
            "five" => "5",
            "six" => "6",
            "seven" => "7",
            "eight" => "8",
            "nine" => "9",
            "ten" => "10",
            "eleven" => "11",
            "twelve" => "12",
            "thirteen" => "13",
            "fourteen" => "14",
            "fifteen" => "15",
            "sixteen" => "16",
            "seventeen" => "17",
            "eighteen" => "18",
            "nineteen" => "19",
            "twenty" => "20",
            "thirty" => "30",
            "forty" => "40",
            "fifty" => "50",
            "sixty" => "60",
            "seventy" => "70",
            "eighty" => "80",
            "ninety" => "90",
            "dr" => "doctor",
            "mr" => "mister",
            "mrs" => "missus",
            "ok" => "okay",
            other => other,
        };
        out.push(w.to_string());
        if dollars_pending {
            out.push("dollars".into());
            dollars_pending = false;
        }
    }
    out
}

/// Levenshtein distance over word sequences.
pub fn edit_distance(a: &[String], b: &[String]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + (x != y) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Returns (errors, reference word count).
pub fn wer_counts(reference: &str, hypothesis: &str) -> (usize, usize) {
    let r = normalize(reference);
    let h = normalize(hypothesis);
    (edit_distance(&r, &h), r.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_money_numbers_punct() {
        assert_eq!(
            normalize("The total is $342, due March 15."),
            normalize("the total is 342 dollars due march fifteen")
        );
        assert_eq!(normalize("1,250 units, up 12%"), vec!["1250", "units", "up", "12", "percent"]);
        assert_eq!(normalize("I'll be ten minutes late"), vec!["ill", "be", "10", "minutes", "late"]);
        assert_eq!(wer_counts("a b c d", "a x c"), (2, 4));
    }
}
