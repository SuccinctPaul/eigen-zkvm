mod memory;

mod utils;
mod wasm_circom;
pub mod witness_calculator;

use crate::r1cs_witness::utils::flat_array;
use num_bigint::BigInt;
use num_traits::{One, Zero};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;

pub fn load_input_for_witness(input_file: &str) -> HashMap<String, Vec<BigInt>> {
    let inputs_str = std::fs::read_to_string(input_file).unwrap();
    let inputs: HashMap<String, Value> = serde_json::from_str(&inputs_str).unwrap();

    inputs
        .iter()
        .map(|(key, value)| {
            let res = match value {
                Value::String(inner) => {
                    vec![BigInt::from_str(inner).unwrap()]
                }
                Value::Bool(inner) => {
                    if *inner {
                        vec![BigInt::one()]
                    } else {
                        vec![BigInt::zero()]
                    }
                }
                Value::Number(inner) => {
                    vec![BigInt::from_str(&inner.to_string()).unwrap()]
                }
                Value::Array(inner) => flat_array(inner),
                _ => panic!("{:?}", value),
            };

            (key.clone(), res)
        })
        .collect::<HashMap<_, _>>()
}
