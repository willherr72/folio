use super::*;

#[test]
fn redaction_numbers_owned_token_corpus_records_reader_contract() {
    use std::{collections::BTreeSet, fmt::Write};
    let mut tokens = BTreeSet::new();
    for token in [
        "0",
        "-0",
        "+0",
        "1.",
        ".5",
        "-.25",
        "+12.125",
        "1000000",
        "-1000000",
        "1000000.0625",
        "1.000000059604644775390626",
        "1.000000059604644775390625",
        "0.9999999701976776123046875",
        "0000000000.123456789",
        "0000000000.25",
    ] {
        tokens.insert(token.to_owned());
    }
    for count in 1..=60 {
        let pattern = "123456789".repeat(7)[..count].to_owned();
        for fraction in [
            pattern,
            format!("{}1", "0".repeat(count - 1)),
            "0".repeat(count),
        ] {
            for prefix in ["0.", "0000000000.", "-000000000.", "+000000000."] {
                let token = format!("{prefix}{fraction}");
                if token.len() <= 64 {
                    tokens.insert(token);
                }
            }
        }
    }
    let mut seed = 271_u32;
    for bits in [1, 2, 0x7ffffd, 0x7ffffe, 0x7fffff, 0x800000, 0x800001] {
        let value = f32::from_bits(bits);
        tokens.insert(value.to_string());
        tokens.insert(format!("{value:.60}"));
    }
    for end in [
        "210", "211", "220", "230", "240", "250", "270", "280", "300",
    ] {
        tokens.insert(format!("0.{}1175494{end}", "0".repeat(37)));
    }
    for _ in 0..512 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let exponent = 1 + (seed % 145);
        let bits = (exponent << 23) | (seed & 0x7fffff);
        let value = f32::from_bits(bits);
        for value in [value, -value] {
            tokens.insert(value.to_string());
            tokens.insert(format!("{value:.30}"));
        }
        tokens.insert(format!("{}.{}", seed % 1000000, seed));
    }
    let mut output = String::from("id\ttoken\taccepted\tvalue\tcanonical\n");
    let mut accepted = 0;
    let mut refused = 0;
    for (index, token) in tokens.into_iter().enumerate() {
        let value = token.parse::<f32>().unwrap();
        let proof = prove_token(token.as_bytes());
        if let Ok(proved) = proof {
            assert_eq!(proved, value);
            accepted += 1;
        } else {
            refused += 1;
        }
        writeln!(
            output,
            "{index}\t{token}\t{}\t{}\t{}",
            proof.is_ok(),
            f64::from(value),
            value
        )
        .unwrap();
    }
    assert!(accepted > 1000 && refused > 100);
    if let Some(path) = std::env::var_os("FOLIO_NUMBER_OUTPUT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("tokens.tsv"), output).unwrap();
    }
}

#[test]
fn redaction_numbers_refuse_reader_dependent_and_underflow_values() {
    for token in [
        format!("0000000000.{}", "123456789".repeat(5)),
        format!("0.{}1", "0".repeat(45)),
        "1.000000059604644775390626".to_owned(),
    ] {
        assert!(prove_token(token.as_bytes()).is_err(), "{token}");
    }
}

#[test]
fn redaction_numbers_keep_common_and_equivalent_long_spellings() {
    for token in [
        "0",
        "-0",
        "+0",
        "1.",
        ".5",
        "-.25",
        "+12.125",
        "0.123456789",
        "0000000000.25",
        "0000000000.123456789",
        "-000000000.25",
        "0.000000000123456789",
        "1000000",
        "-1000000",
        "0.000000000000000000000000000000000000000000",
    ] {
        assert_eq!(
            prove_token(token.as_bytes()).unwrap(),
            token.parse::<f32>().unwrap(),
            "{token}"
        );
    }
}

#[test]
fn redaction_numbers_enforce_grammar_and_budgets_before_conversion() {
    for token in [
        "", "+", ".", "--1", "1e2", "NaN", "inf", "1.2.3", "1 2", "1000001", "-1000001",
    ] {
        assert!(prove_token(token.as_bytes()).is_err(), "{token}");
    }
    assert!(prove_token("0".repeat(65).as_bytes()).is_err());
}
