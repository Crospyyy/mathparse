use crate::libraries::storing::FormulaStore;
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
                        println!("~= {}", result.value());
                    } else {
                        println!("= {}", result.value());
                    }
                },
                Err(s) => {
                    println!("Error: {}", s);
                },
            }
        }
    }
}
