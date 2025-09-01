use std::fmt::Display;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ExpressionFunType {
    Sin,
    Asin,
    Cos,
    Acos,
    Tan,
    Atan,
    Floor,
    Round,
    Rem,
    Log2,
    SinWithRadians,
}

impl From<ExpressionFunType> for ExprValue<ExpressionFunType> {
    fn from(fun_type: ExpressionFunType) -> Self {
        ExprValue::Native(fun_type)
    }
}

impl From<ExpressionNumType> for ExprValue<ExpressionNumType> {
    fn from(num_type: ExpressionNumType) -> Self {
        ExprValue::Native(num_type)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ExpressionNumType {
    Pi,
    E,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprValue<T: Display> {
    Native(T),
    Custom(String),
}

impl<T: PartialEq + Display> ExprValue<T> {
    pub fn same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (ExprValue::Native(a), ExprValue::Native(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<&str> for ExpressionNumType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pi" => Ok(ExpressionNumType::Pi),
            "e" => Ok(ExpressionNumType::E),
            _ => Err(format!("Unknown ExpressionNumType: {}", value)),
        }
    }
}

impl TryFrom<&str> for ExpressionFunType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "sin" => Ok(ExpressionFunType::Sin),
            "asin" => Ok(ExpressionFunType::Asin),
            "cos" => Ok(ExpressionFunType::Cos),
            "acos" => Ok(ExpressionFunType::Acos),
            "tan" => Ok(ExpressionFunType::Tan),
            "atan" => Ok(ExpressionFunType::Atan),
            "floor" => Ok(ExpressionFunType::Floor),
            "round" => Ok(ExpressionFunType::Round),
            "rem" => Ok(ExpressionFunType::Rem),
            "log2" => Ok(ExpressionFunType::Log2),
            _ => Err(format!("Unknown ExpressionFunType: {}", value)),
        }
    }
}

impl<T: for<'a> TryFrom<&'a str> + Display> From<&str> for ExprValue<T> {
    fn from(value: &str) -> Self {
        match T::try_from(value) {
            Ok(fun_type) => ExprValue::Native(fun_type),
            Err(_) => ExprValue::Custom(value.to_string()),
        }
    }
}
