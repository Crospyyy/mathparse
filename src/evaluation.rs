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

    fn get_num_value(&self) -> Option<f64> {
        if let Element::Number(num) = self { Some(*num) } else { None }
    }

   // pub fn expanded_with_symbols(symbols: &FormulaStore)->Option<Element>{
    // 
    // }
    
    // pub fn eval_with_defined(&self, stored: &FormulaStore) -> Option<Element> {
    //     match self {
    //         Element::Brackets(_) | Element::String(_) => None,
    //         Element::Plus(elements) => {
    //             let mut sum = 0.0;
    //             for n in elements {
    //                 sum += n.eval_with_defined(stored)?.get_num_value()?;
    //             }
    //             Some(Element::Number(sum))
    //         },
    //         Element::Multiply(elements) => {
    //             let mut product = 1.0;
    //             for n in elements {
    //                 product *= n.eval_with_defined(stored)?.get_num_value()?;
    //             }
    //             Some(Element::Number(product))
    //         },
    //
    //         Element::Pow(a, b) => Some(Element::Number(
    //             a.eval_with_defined(stored)?
    //                 .get_num_value()?
    //                 .powf(b.eval_with_defined(stored)?.get_num_value()?),
    //         )),
    //         Element::Negate(x) => {
    //             Some(Element::Number(-x.eval_with_defined(stored)?.get_num_value()?))
    //         },
    //         Element::Number(num) => Some(self.clone()),
    //         Element::Function { name, arguments } => {
    //             let values = arguments.iter().map(|a| a.eval_with_defined(stored)).collect::<Option<Vec<_>>>();
    //
    //             todo!()
    //             //stored.get_formulas().get(name)?
    //         },
    //         Element::Variable(_) => {},
    //         Element::VariableOrFunction(_) => {},
    //     }
    // }
}

// impl FormulaStore {
//     pub fn eval(&self, name: &str) -> Option<f64> {
//         let formula = Element::parse(name)?;
//         let sig = Signatures::generate_needed_elements_of_formula(&formula);
//         if !self.get_signatures().contains_all_of(&sig) {
//             return None;
//         }
//         formula.eval_with_defined(self)
//     }
// }

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
