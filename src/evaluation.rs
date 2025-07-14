use crate::Element;
use crate::parsing::signature::{Signature, Signatures};
use crate::storing::FormulaStore;

impl Element {
    pub fn eval(&self) -> Option<f64> {
        match self {
            Element::Brackets(_)
            | Element::String(_)
            | Element::Variable(_)
            | Element::Function { .. }
            | Element::VariableOrFunction(_) => None,

            Element::Plus(elements) => {
                let mut sum = 0.0;
                for n in elements {
                    sum += n.eval()?;
                }
                Some(sum)
            },

            Element::Multiply(elements) => {
                let mut product = 1.0;
                for n in elements {
                    product *= n.eval()?;
                }
                Some(product)
            },

            Element::Negate(e) => e.eval().map(|n| -n),
            Element::Number(n) => Some(*n),
            Element::Pow(b, e) => Some(b.eval()?.powf(e.eval()?)),
        }
    }

    pub fn eval_with_defined(&self, stored: &FormulaStore) -> Option<f64> {
        todo!()
    }
}

impl FormulaStore {
    pub fn eval(&self, name: &str) -> Option<f64> {
        let formula = Element::parse(name)?;
        let sig = Signatures::generate_needed_elements_of_formula(&formula);
        if !self.get_signatures().contains_all_of(&sig) {
            return None;
        }
        formula.eval_with_defined(self)
    }
}

impl Signatures {
    fn contains_all_of(&self, signatures: &Signatures) -> bool {
        for (name, sig) in &signatures.0 {
            if self.0.get(name).is_none_or(|self_sig| !sig.could_be(self_sig)) {
                return false;
            }
        }
        true
    }
}
