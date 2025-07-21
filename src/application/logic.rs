use crate::libraries::storing::FormulaStore;
use num::BigRational;
use std::io::{Write, stdin, stdout};

pub fn run_formula_evaluator() {
    let mut store = FormulaStore::new_empty();

    loop {
        println!("Input a formula:");
        stdout().flush().unwrap();
        let mut line = "".to_owned();
        let _ = stdin().read_line(&mut line);
        line = line.trim().to_string();
        if line.contains('=') {
            let result = store.add_symbol_from_string(&line);
            println!("{:?}", result);
        } else {
            let result1 = store.safe_eval(&line);
            match result1 {
                Ok(result) => {
                    if result.is_lossy() {
                        println!("~= {}", convert_num_to_string(result.value()));
                    } else {
                        println!("= {}", convert_num_to_string(result.value()));
                    }
                },
                Err(s) => {
                    println!("Error: {}", s);
                },
            }
        }
    }
}

pub fn convert_num_to_string(num: f64) -> String {
    if num < 1.0 {
        num.to_string()
    } else {
        let mut string = (num - num.floor()).to_string();
        string.remove(0);
        let ratio = BigRational::from_float(num.floor()).unwrap();
        dbg!(&ratio);
        string.insert_str(0, ratio.numer().to_string().as_str());
        string
    }
}

#[test]
fn conv() {
    let string = convert_num_to_string(123456778.0);
    dbg!(string);
}
