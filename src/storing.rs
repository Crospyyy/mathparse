use crate::Element;
use crate::parsing::signature::Signatures;
use std::collections::HashMap;

pub struct FormulaStore {
    signatures: Signatures,
    formulas: HashMap<String, Element>,
}

impl FormulaStore {
    pub(crate) fn new_empty() -> Self {
        FormulaStore { signatures: Signatures::new_empty(), formulas: HashMap::new() }
    }

    pub(crate) fn add_symbol_from_string(&mut self, string: &str) -> Result<(), String> {
        let (sig, def) = string.split_once("=").ok_or("String doesn't contain '='".to_owned())?;
        let sig = Element::parse(sig).ok_or("First formula could not be parsed")?;
        println!("Signature: {:?}", sig);
        let def = Element::parse(def).ok_or("Second formula could not be parsed")?;
        println!("Definition: {:?}", def);

        match self.signatures.add_symbol_from_function_signature_and_definition(sig, def.clone()) {
            Ok(name) => {
                self.formulas.insert(name, def);
                Ok(())
            },
            Err(err) => Err(format!("Could not add symbol: {}", err)),
        }
    }

    pub(crate) fn get_signatures(&self) -> &Signatures {
        &self.signatures
    }

    pub(crate) fn get_formulas(&self) -> &HashMap<String, Element> {
        &self.formulas
    }
}
