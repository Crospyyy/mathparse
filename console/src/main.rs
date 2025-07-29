fn main() {
    println!("Hello, world!");
}
use library::operations::create_default_context;
use library::storing::FormulaStore;
use num::{BigInt, BigRational};
use std::io::{Write, stdin, stdout};

pub fn run_formula_evaluator() {
    let mut store = FormulaStore::new_empty();
    store.define_default_symbols().unwrap();
    let ctx = &mut create_default_context();

    loop {
        println!("\nInput a formula:");
        stdout().flush().unwrap();
        let mut line = "".to_owned();
        let _ = stdin().read_line(&mut line);
        line = line.trim().to_string();
        if line.contains('=') {
            let result = store.add_symbol_from_string(&line, false);
            match result {
                Ok(name) => println!("Added new symbol '{}'", name),
                Err(s) => println!("Error: {}", s),
            }
        } else {
            let result1 = store.eval(&line, ctx);
            match result1 {
                Ok(result) => {
                    if result.is_exact() {
                        println!("= {}", result.to_string_default_rounding(ctx));
                    } else {
                        println!("~= {}", result.to_string_default_rounding(ctx));
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
    if num < 1.0 || !num.is_finite() {
        num.to_string()
    } else {
        let mut string = (num - num.floor()).to_string();
        string.remove(0);
        let ratio = BigRational::from_float(num.floor()).unwrap();
        assert_eq!(*ratio.denom(), BigInt::from(1));
        string.insert_str(0, ratio.numer().to_string().as_str());
        string
    }
}

#[test]
fn conv() {
    let string = convert_num_to_string(123456778.0);
    dbg!(string);
}
