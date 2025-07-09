use crate::Element;

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
}
