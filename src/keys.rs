//! Vim-style link hints (home-row labels).

const ALPHA: &[u8] = b"asdfghjkl";

pub fn generate_hints(n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    if n <= ALPHA.len() {
        return ALPHA
            .iter()
            .take(n)
            .map(|c| (*c as char).to_string())
            .collect();
    }
    let mut out = Vec::with_capacity(n);
    for a in ALPHA {
        for b in ALPHA {
            out.push(format!("{}{}", *a as char, *b as char));
            if out.len() == n {
                return out;
            }
        }
    }
    out
}

pub fn matches<'a>(hints: &'a [(String, HintTarget)], typed: &str) -> Vec<&'a (String, HintTarget)> {
    hints
        .iter()
        .filter(|(h, _)| h.starts_with(typed))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintTarget {
    Link(u32),
    Result(usize),
    Nav(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_then_double() {
        let one = generate_hints(3);
        assert_eq!(one, vec!["a", "s", "d"]);
        let many = generate_hints(12);
        assert_eq!(many.len(), 12);
        assert!(many[0].len() == 2);
        assert!(many.iter().all(|h| h.chars().all(|c| ALPHA.contains(&(c as u8)))));
    }
}
