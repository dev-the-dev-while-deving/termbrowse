//! TeX / MathML → Unicode preview. No MathJax, no Chrome.

pub fn looks_like_tex(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 2 {
        return false;
    }
    t.contains("\\frac")
        || t.contains("\\sqrt")
        || t.contains("\\sum")
        || t.contains("\\int")
        || t.contains("displaystyle")
        || t.contains("^{")
        || t.contains("_{")
        || (t.contains('\\') && t.chars().any(|c| c.is_ascii_alphabetic()))
}

pub fn tex_to_unicode(input: &str) -> String {
    let s = strip_wrappers(input);
    let rendered = render(&s);
    collapse_ws(&rendered)
}

fn strip_wrappers(s: &str) -> String {
    let mut t = s.trim().to_string();
    for _ in 0..4 {
        let next = t
            .trim()
            .trim_start_matches("\\displaystyle")
            .trim_start_matches("\\textstyle")
            .trim_start_matches("\\text{")
            .trim()
            .to_string();
        let next = next
            .strip_prefix("{\\displaystyle")
            .or_else(|| next.strip_prefix("{"))
            .unwrap_or(&next)
            .trim()
            .trim_end_matches('}')
            .trim()
            .to_string();
        if next == t {
            break;
        }
        t = next;
    }
    t
}

fn render(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                i += 1;
                let (cmd, rest) = read_command(&chars, i);
                i = rest;
                match cmd.as_str() {
                    "frac" | "dfrac" | "tfrac" => {
                        let (num, n2) = take_group(&chars, i);
                        let (den, n3) = take_group(&chars, n2);
                        i = n3;
                        out.push_str(&render(&num));
                        out.push('⁄');
                        out.push_str(&render(&den));
                    }
                    "sqrt" => {
                        let (inner, n2) = take_group(&chars, i);
                        i = n2;
                        out.push('√');
                        let body = render(&inner);
                        if body.chars().count() > 1 {
                            out.push('(');
                            out.push_str(&body);
                            out.push(')');
                        } else {
                            out.push_str(&body);
                        }
                    }
                    "sum" => out.push('∑'),
                    "prod" => out.push('∏'),
                    "int" => out.push('∫'),
                    "iint" => out.push('∬'),
                    "oint" => out.push('∮'),
                    "infty" | "infinity" => out.push('∞'),
                    "pm" => out.push('±'),
                    "mp" => out.push('∓'),
                    "times" => out.push('×'),
                    "cdot" | "bullet" => out.push('·'),
                    "div" => out.push('÷'),
                    "leq" | "le" => out.push('≤'),
                    "geq" | "ge" => out.push('≥'),
                    "neq" | "ne" => out.push('≠'),
                    "approx" => out.push('≈'),
                    "equiv" => out.push('≡'),
                    "sim" => out.push('∼'),
                    "to" | "rightarrow" => out.push('→'),
                    "leftarrow" => out.push('←'),
                    "leftrightarrow" => out.push('↔'),
                    "Rightarrow" => out.push('⇒'),
                    "in" => out.push('∈'),
                    "notin" => out.push('∉'),
                    "subset" => out.push('⊂'),
                    "subseteq" => out.push('⊆'),
                    "forall" => out.push('∀'),
                    "exists" => out.push('∃'),
                    "partial" => out.push('∂'),
                    "nabla" => out.push('∇'),
                    "emptyset" => out.push('∅'),
                    "ldots" | "dots" | "cdots" => out.push('…'),
                    "hbar" => out.push('ℏ'),
                    "ell" => out.push('ℓ'),
                    "Re" => out.push('ℜ'),
                    "Im" => out.push('ℑ'),
                    "quad" | "qquad" | "," | ";" | " " => out.push(' '),
                    "left" | "right" | "big" | "Big" | "mathrm" | "operatorname" | "text"
                    | "textbf" | "mathit" | "mathbf" | "mathcal" | "operatorname*" => {
                        let (inner, n2) = take_group(&chars, i);
                        i = n2;
                        out.push_str(&render(&inner));
                    }
                    "overline" => {
                        let (inner, n2) = take_group(&chars, i);
                        i = n2;
                        out.push_str(&render(&inner));
                        out.push('\u{0305}');
                    }
                    other => {
                        if let Some(g) = greek(other) {
                            out.push(g);
                        } else if other == "\\" {
                            out.push('\n');
                        } else {
                            out.push_str(other);
                        }
                    }
                }
            }
            '^' => {
                let (grp, n2) = take_group(&chars, i + 1);
                i = n2;
                out.push_str(&script(&render(&grp), true));
            }
            '_' => {
                let (grp, n2) = take_group(&chars, i + 1);
                i = n2;
                out.push_str(&script(&render(&grp), false));
            }
            '{' | '}' => i += 1,
            '&' => {
                out.push('\t');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn read_command(chars: &[char], mut i: usize) -> (String, usize) {
    if i >= chars.len() {
        return (String::new(), i);
    }
    if !chars[i].is_ascii_alphabetic() {
        let c = chars[i];
        return (c.to_string(), i + 1);
    }
    let start = i;
    while i < chars.len() && chars[i].is_ascii_alphabetic() {
        i += 1;
    }
    (chars[start..i].iter().collect(), i)
}

fn take_group(chars: &[char], mut i: usize) -> (String, usize) {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() {
        return (String::new(), i);
    }
    if chars[i] != '{' {
        return (chars[i].to_string(), i + 1);
    }
    i += 1;
    let start = i;
    let mut depth = 1;
    while i < chars.len() && depth > 0 {
        match chars[i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            break;
        }
        i += 1;
    }
    let inner: String = chars[start..i].iter().collect();
    if i < chars.len() && chars[i] == '}' {
        i += 1;
    }
    (inner, i)
}

fn script(s: &str, super_script: bool) -> String {
    s.chars()
        .map(|c| map_script(c, super_script).unwrap_or(c))
        .collect()
}

fn map_script(c: char, super_script: bool) -> Option<char> {
    if super_script {
        Some(match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            '+' => '⁺',
            '-' => '⁻',
            '=' => '⁼',
            '(' => '⁽',
            ')' => '⁾',
            'n' => 'ⁿ',
            'i' => 'ⁱ',
            _ => return None,
        })
    } else {
        Some(match c {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            '+' => '₊',
            '-' => '₋',
            '=' => '₌',
            '(' => '₍',
            ')' => '₎',
            'a' => 'ₐ',
            'e' => 'ₑ',
            'o' => 'ₒ',
            'x' => 'ₓ',
            'n' => 'ₙ',
            'i' => 'ᵢ',
            'k' => 'ₖ',
            'm' => 'ₘ',
            't' => 'ₜ',
            _ => return None,
        })
    }
}

fn greek(name: &str) -> Option<char> {
    Some(match name {
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "epsilon" | "varepsilon" => 'ε',
        "zeta" => 'ζ',
        "eta" => 'η',
        "theta" => 'θ',
        "iota" => 'ι',
        "kappa" => 'κ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "nu" => 'ν',
        "xi" => 'ξ',
        "pi" => 'π',
        "rho" => 'ρ',
        "sigma" => 'σ',
        "tau" => 'τ',
        "upsilon" => 'υ',
        "phi" | "varphi" => 'φ',
        "chi" => 'χ',
        "psi" => 'ψ',
        "omega" => 'ω',
        "Gamma" => 'Γ',
        "Delta" => 'Δ',
        "Theta" => 'Θ',
        "Lambda" => 'Λ',
        "Xi" => 'Ξ',
        "Pi" => 'Π',
        "Sigma" => 'Σ',
        "Phi" => 'Φ',
        "Psi" => 'Ψ',
        "Omega" => 'Ω',
        _ => return None,
    })
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pull `$...$`, `$$...$$`, `\(...\)`, `\[...\]` out of prose.
pub fn preview_inline_math(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '$' && chars[i + 1] == '$' {
            if let Some(end) = find_close(&chars, i + 2, "$$") {
                let tex: String = chars[i + 2..end].iter().collect();
                out.push_str(&tex_to_unicode(&tex));
                i = end + 2;
                continue;
            }
        }
        if chars[i] == '$' {
            if let Some(end) = find_close(&chars, i + 1, "$") {
                let tex: String = chars[i + 1..end].iter().collect();
                out.push_str(&tex_to_unicode(&tex));
                i = end + 1;
                continue;
            }
        }
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '(' {
            if let Some(end) = find_seq(&chars, i + 2, &['\\', ')']) {
                let tex: String = chars[i + 2..end].iter().collect();
                out.push_str(&tex_to_unicode(&tex));
                i = end + 2;
                continue;
            }
        }
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '[' {
            if let Some(end) = find_seq(&chars, i + 2, &['\\', ']']) {
                let tex: String = chars[i + 2..end].iter().collect();
                out.push_str(&tex_to_unicode(&tex));
                i = end + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn find_close(chars: &[char], from: usize, delim: &str) -> Option<usize> {
    let d: Vec<char> = delim.chars().collect();
    let mut i = from;
    while i + d.len() <= chars.len() {
        if chars[i..i + d.len()] == d[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    let mut i = from;
    while i + seq.len() <= chars.len() {
        if &chars[i..i + seq.len()] == seq {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_mass_energy() {
        let u = tex_to_unicode("E = mc^{2}");
        assert!(u.contains('²'), "{u}");
        assert!(u.contains('E'), "{u}");
    }

    #[test]
    fn preview_fraction_and_greek() {
        let u = tex_to_unicode("\\frac{\\alpha}{\\beta}");
        assert!(u.contains('α'), "{u}");
        assert!(u.contains('⁄'), "{u}");
        assert!(u.contains('β'), "{u}");
    }

    #[test]
    fn dollar_math_in_prose() {
        let s = preview_inline_math("energy $E=mc^{2}$ holds");
        assert!(s.contains('²'));
        assert!(!s.contains('$'));
    }
}
