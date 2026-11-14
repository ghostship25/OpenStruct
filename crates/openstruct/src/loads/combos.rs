//! Load combination editor abstractions for Phase 3.

use super::ComboType;
use super::LoadCombination;

/// One term of a combination equation: a factor applied to a case or pattern name.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboTerm {
    pub name: String,
    pub factor: f64,
}

/// Parse a combination equation such as `1.2*DEAD + 1.6*LIVE`, `DEAD - 0.9*WIND`
/// or `2*CASE1`. A term without a factor means a factor of 1.
pub fn parse_equation(equation: &str) -> Result<Vec<ComboTerm>, String> {
    let mut terms = Vec::new();
    let mut rest = equation.trim();
    if rest.is_empty() {
        return Err("the equation is empty".to_string());
    }
    let mut sign = 1.0;
    if let Some(stripped) = rest.strip_prefix('+') {
        rest = stripped.trim_start();
    } else if let Some(stripped) = rest.strip_prefix('-') {
        sign = -1.0;
        rest = stripped.trim_start();
    }
    loop {
        let (term_text, remainder) = match rest.find(['+', '-']) {
            Some(index) => (&rest[..index], Some(&rest[index..])),
            None => (rest, None),
        };
        let term_text = term_text.trim();
        if term_text.is_empty() {
            return Err(format!("missing a term in '{equation}'"));
        }
        let (factor_text, name) = match term_text.split_once('*') {
            Some((factor, name)) => (factor.trim(), name.trim()),
            None => ("1", term_text),
        };
        let factor: f64 = factor_text
            .parse()
            .map_err(|_| format!("'{factor_text}' is not a number"))?;
        if name.is_empty() {
            return Err(format!("missing a case name in '{term_text}'"));
        }
        terms.push(ComboTerm { name: name.to_string(), factor: sign * factor });
        match remainder {
            None => break,
            Some(after) => {
                sign = if after.starts_with('-') { -1.0 } else { 1.0 };
                rest = after[1..].trim_start();
                if rest.is_empty() {
                    return Err(format!("trailing operator in '{equation}'"));
                }
            }
        }
    }
    Ok(terms)
}

/// Default code-based hint combinations presented until the user fills in project-specific
/// combinations from Phase 3's load combination editor.
pub fn code_default_hints() -> Vec<(String, ComboType, String)> {
    vec![
        ("1.4D".into(), ComboType::LinearAdd, "1.4*DEAD".into()),
        ("1.2D + 1.6L".into(), ComboType::LinearAdd, "1.2*DEAD + 1.6*LIVE".into()),
        ("1.2D + 1.0L + 1.0W".into(), ComboType::LinearAdd, "1.2*DEAD + 1.0*LIVE + 1.0*WIND".into()),
        ("1.2D + 1.0L + 1.0E".into(), ComboType::LinearAdd, "1.2*DEAD + 1.0*LIVE + 1.0*QUAKE".into()),
        ("1.2D + 1.6W".into(), ComboType::LinearAdd, "1.2*DEAD + 1.6*WIND".into()),
        ("1.2D + 1.0E + 0.5L".into(), ComboType::LinearAdd, "1.2*DEAD + 1.0*QUAKE + 0.5*LIVE".into()),
    ]
}

/// Minimal combination builder used by the Phase 3 dialogs.
pub struct ComboBuilder {
    pub id: usize,
    pub name: String,
    pub equation: String,
    pub combo_type: ComboType,
    pub factors: Vec<(usize, f64)>,
}

impl ComboBuilder {
    pub fn new(id: usize, name: String, equation: String, combo_type: ComboType) -> Self {
        Self {
            id,
            name,
            equation,
            combo_type,
            factors: Vec::new(),
        }
    }

    pub fn add_factor(mut self, pattern_id: usize, factor: f64) -> Self {
        self.factors.push((pattern_id, factor));
        self
    }

    pub fn build(self) -> LoadCombination {
        LoadCombination {
            id: self.id,
            name: self.name,
            equation: self.equation,
            combo_type: self.combo_type,
            factors: self.factors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equation_terms_are_parsed_with_factors_and_signs() {
        let terms = parse_equation("1.2*DEAD + 1.6*LIVE").unwrap();
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0], ComboTerm { name: "DEAD".into(), factor: 1.2 });
        assert_eq!(terms[1], ComboTerm { name: "LIVE".into(), factor: 1.6 });

        let terms = parse_equation("DEAD - 0.9*WIND").unwrap();
        assert_eq!(terms[0], ComboTerm { name: "DEAD".into(), factor: 1.0 });
        assert_eq!(terms[1], ComboTerm { name: "WIND".into(), factor: -0.9 });
    }

    #[test]
    fn malformed_equations_are_rejected() {
        assert!(parse_equation("").is_err());
        assert!(parse_equation("1.2*").is_err());
        assert!(parse_equation("abc*DEAD").is_err());
        assert!(parse_equation("1.4*DEAD +").is_err());
    }

    #[test]
    fn linear_add_default_is_usable() {
        let combo = ComboBuilder::new(1, "1.4D".into(), "1.4*DEAD".into(), ComboType::LinearAdd)
            .add_factor(1, 1.4)
            .build();
        assert_eq!(combo.name, "1.4D");
        assert_eq!(combo.combo_type, ComboType::LinearAdd);
        assert_eq!(combo.factors, &[(1, 1.4)]);
    }
}
