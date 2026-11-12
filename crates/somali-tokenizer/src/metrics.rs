//! Small numeric helpers shared by the benchmark and corpus statistics.

use std::collections::HashMap;

/// Round to `digits` decimals the way Python's `round(x, digits)` does: correctly rounded
/// from the exact binary value, ties to even.
#[must_use]
pub fn round_to(value: f64, digits: usize) -> f64 {
    format!("{value:.digits$}").parse().unwrap_or(value)
}

/// `numerator / denominator` rounded, or `0.0` when the denominator is zero.
#[must_use]
pub fn ratio(numerator: u64, denominator: u64, digits: usize) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    round_to(numerator as f64 / denominator as f64, digits)
}

/// Per-source counters that remember first-seen order, so ties sort the way Python's stable
/// sort over an insertion-ordered dict does.
#[derive(Debug, Clone, Default)]
pub struct BySource<T> {
    index: HashMap<String, usize>,
    entries: Vec<(String, T)>,
}

impl<T: Default> BySource<T> {
    /// The counters for `source`, created on first use.
    pub fn entry(&mut self, source: &str) -> &mut T {
        let position = match self.index.get(source) {
            Some(&position) => position,
            None => {
                self.entries.push((source.to_owned(), T::default()));
                let position = self.entries.len() - 1;
                self.index.insert(source.to_owned(), position);
                position
            }
        };
        &mut self.entries[position].1
    }

    /// Entries sorted by `key` descending; ties keep first-seen order.
    pub fn sorted_by_desc<K: Ord>(&self, key: impl Fn(&T) -> K) -> Vec<(&str, &T)> {
        let mut sorted: Vec<(&str, &T)> = self
            .entries
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect();
        sorted.sort_by(|a, b| key(b.1).cmp(&key(a.1)));
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_matches_python() {
        assert_eq!(round_to(1.38295, 4), 1.3829); // 1.38295 is just below the tie
        assert_eq!(round_to(1.38305, 4), 1.383);
        assert_eq!(round_to(2.675, 2), 2.67); // 2.675 is 2.67499999... in binary
        assert_eq!(round_to(0.125, 2), 0.12); // exact tie goes to even
    }

    #[test]
    fn ties_keep_first_seen_order() {
        let mut counts: BySource<u64> = BySource::default();
        *counts.entry("b") += 1;
        *counts.entry("a") += 1;
        *counts.entry("c") += 5;
        let order: Vec<&str> = counts
            .sorted_by_desc(|n| *n)
            .into_iter()
            .map(|e| e.0)
            .collect();
        assert_eq!(order, ["c", "b", "a"]);
    }
}
