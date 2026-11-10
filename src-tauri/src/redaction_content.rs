//! Conservative content admission for the private redaction kernel.
use crate::{EngineError, EngineResult};
use lopdf::{
    content::{Content, Operation},
    Object,
};

pub(super) fn invalid() -> EngineError {
    EngineError::InvalidRequest(
        "This PDF is outside the private structural redaction subset.".into(),
    )
}
pub(super) fn require(ok: bool) -> EngineResult<()> {
    if ok {
        Ok(())
    } else {
        Err(invalid())
    }
}
pub(super) fn number(o: &Object) -> EngineResult<f64> {
    let n = match o {
        Object::Integer(n) => *n as f64,
        Object::Real(n) => *n as f64,
        _ => return Err(invalid()),
    };
    require(n.is_finite() && n.abs() <= 1_000_000.0)?;
    Ok(n)
}
#[derive(Default)]
pub(super) struct ContentBudget {
    tokens: usize,
    operations: usize,
    text: usize,
    decoded: usize,
}
pub(super) struct Block {
    pub index: usize,
    pub text: String,
    pub shows: Vec<usize>,
}
pub(super) struct Program {
    pub operations: Vec<Operation>,
    pub blocks: Vec<Block>,
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    #[test]
    fn redaction_preflight_refuses_numeric_reader_disagreement_but_skips_strings() {
        let token=format!("0000000000.{}","123456789".repeat(5));
        assert!(preflight(format!("{token} 0 0 1 0 0 cm").as_bytes(), &mut ContentBudget::default()).is_err());
        assert!(preflight(format!("[{token}] 0 d").as_bytes(), &mut ContentBudget::default()).is_err());
        assert!(preflight(format!("BT ({token}) Tj ET % {token}").as_bytes(), &mut ContentBudget::default()).is_ok());
    }
    #[test]
    fn redaction_preflight_rejects_ambiguous_numeric_tokens_before_decoding() {
        for lexeme in ["0", "+1", "-0", "1.", ".5", "-.25"] {
            assert!(
                preflight(
                    format!("{lexeme} g").as_bytes(),
                    &mut ContentBudget::default()
                )
                .is_ok(),
                "{lexeme}"
            );
        }
        for lexeme in ["+1+1", "1.2.3", "--1", "+", ".", "1-2"] {
            let data = format!("[{lexeme}] 0 d");
            assert!(
                preflight(data.as_bytes(), &mut ContentBudget::default()).is_err(),
                "{lexeme}"
            );
        }
        let data = format!("[{}] 0 d", format!("{} ", "+1".repeat(32)).repeat(129));
        assert!(preflight(data.as_bytes(), &mut ContentBudget::default()).is_err());
    }
}

fn white(c: u8) -> bool {
    matches!(c, 0 | 9 | 10 | 12 | 13 | 32)
}
fn delimiter(c: u8) -> bool {
    white(c) || b"()<>[]{}/%".contains(&c)
}

fn operator(s: &[u8]) -> bool {
    matches!(
        s,
        b"q" | b"Q"
            | b"gs"
            | b"Do"
            | b"cm"
            | b"w"
            | b"J"
            | b"j"
            | b"M"
            | b"d"
            | b"g"
            | b"G"
            | b"rg"
            | b"RG"
            | b"k"
            | b"K"
            | b"m"
            | b"l"
            | b"c"
            | b"v"
            | b"y"
            | b"h"
            | b"re"
            | b"S"
            | b"s"
            | b"f"
            | b"F"
            | b"f*"
            | b"B"
            | b"B*"
            | b"b"
            | b"b*"
            | b"n"
            | b"W"
            | b"W*"
            | b"BT"
            | b"ET"
            | b"Tf"
            | b"Tm"
            | b"Td"
            | b"TD"
            | b"T*"
            | b"Tc"
            | b"Tw"
            | b"TL"
            | b"Tz"
            | b"Ts"
            | b"Tr"
            | b"Tj"
            | b"TJ"
            | b"'"
            | b"\""
    )
}

// Runs BEFORE lopdf's allocating decoder, especially its inline-image parser.
// Arrays cannot nest: admitted TJ and dash arrays only contain scalars.
pub(super) fn preflight(data: &[u8], budget: &mut ContentBudget) -> EngineResult<()> {
    let mut i = 0;
    let mut operands = 0;
    let mut array: Option<usize> = None;
    while i < data.len() {
        if white(data[i]) {
            i += 1;
            continue;
        }
        if data[i] == b'%' {
            while i < data.len() && !matches!(data[i], 10 | 13) {
                i += 1;
            }
            continue;
        }
        budget.tokens += 1;
        require(budget.tokens <= 500_000)?;
        let start = i;
        let mut is_operand = true;
        match data[i] {
            b'[' => {
                require(array.is_none())?;
                array = Some(0);
                i += 1;
                operands += 1;
                require(operands <= 16)?;
                continue;
            }
            b']' => {
                require(array.take().is_some())?;
                i += 1;
                continue;
            }
            b'(' => {
                i += 1;
                let mut depth = 1;
                while i < data.len() && depth > 0 {
                    match data[i] {
                        b'\\' => {
                            i += 1;
                            require(i < data.len())?;
                            if data[i] == 13 && data.get(i + 1) == Some(&10) {
                                i += 1;
                            }
                        }
                        b'(' => {
                            depth += 1;
                            require(depth <= 64)?;
                        }
                        b')' => depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                require(depth == 0 && i - start <= 8 * 1024 * 1024)?;
            }
            b'<' => {
                i += 1;
                require(data.get(i) != Some(&b'<'))?;
                while i < data.len() && data[i] != b'>' {
                    require(white(data[i]) || data[i].is_ascii_hexdigit())?;
                    i += 1;
                }
                require(i < data.len())?;
                i += 1;
            }
            b'/' => {
                i += 1;
                while i < data.len() && !delimiter(data[i]) {
                    i += 1;
                }
                require(i - start <= 256)?;
            }
            c if !delimiter(c) => {
                i += 1;
                while i < data.len() && !delimiter(data[i]) {
                    i += 1;
                }
                let token = &data[start..i];
                require(token.len() <= 64)?;
                if operator(token) {
                    require(array.is_none())?;
                    budget.operations += 1;
                    require(budget.operations <= 100_000)?;
                    operands = 0;
                    is_operand = false;
                } else {
                    // No BI, dictionaries, booleans or indirect references reach the decoder.
                    crate::redaction_numbers::prove_token(token)?;
                }
            }
            _ => return Err(invalid()),
        }
        if is_operand {
            if let Some(count) = array.as_mut() {
                *count += 1;
                require(*count <= 4096)?;
            } else {
                operands += 1;
                require(operands <= 16)?;
            }
        }
    }
    require(array.is_none() && operands == 0)
}
fn numbers(args: &[Object], count: usize) -> EngineResult<()> {
    require(args.len() == count)?;
    for a in args {
        number(a)?;
    }
    Ok(())
}
pub(super) trait TextFonts {
    fn select(&mut self, alias: &[u8]) -> EngineResult<usize>;
    fn decode(&self, index: usize, bytes: &[u8]) -> EngineResult<String>;
}
fn text(
    s: &Object,
    block: &mut Block,
    budget: &mut ContentBudget,
    fonts: &impl TextFonts,
    font: usize,
) -> EngineResult<()> {
    let s = s.as_str().map_err(|_| invalid())?;

    budget.text = budget.text.checked_add(s.len()).ok_or_else(invalid)?;
    require(budget.text <= 1024 * 1024)?;
    let decoded = fonts.decode(font, s)?;
    require(decoded.len() <= 1024 * 1024)?;
    budget.decoded = budget
        .decoded
        .checked_add(decoded.len())
        .ok_or_else(invalid)?;
    require(budget.decoded <= 1024 * 1024)?;
    block.text.push_str(&decoded);
    Ok(())
}
pub(super) fn canonicalize(
    data: &[u8],
    fonts: &mut impl TextFonts,
    graphics: &mut crate::redaction_graphics::PageStates<'_, '_>,
    images: &mut crate::redaction_images::PageImages<'_, '_>,
    budget: &mut ContentBudget,
) -> EngineResult<Program> {
    preflight(data, budget)?;
    let decoded = Content::decode_strict(data).map_err(|_| invalid())?;
    let operations = images.normalize_operations(decoded.operations)?;
    let mut program = Program {
        operations: vec![],
        blocks: vec![],
    };
    let mut current: Option<Block> = None;
    let mut ordinal = 0;
    let mut has_font = None;
    let mut stack = vec![];
    for mut op in operations {
        let a = &op.operands;
        require(a.len() <= 16)?;
        match op.operator.as_str() {
            "Do" => {
                require(current.is_none() && a.len() == 1)?;
                // The prepass already resolved this alias and verified any
                // following owned paint; do not resolve the canonical alias
                // against the original resource dictionary a second time.
            }
            "gs" => {
                require(a.len() == 1)?;
                let index = graphics.select(a[0].as_name().map_err(|_| invalid())?)?;
                op.operands[0] = Object::Name(format!("G{index}").into_bytes());
            }
            "BT" => {
                numbers(a, 0)?;
                require(current.is_none())?;
                current = Some(Block {
                    index: ordinal,
                    text: String::new(),
                    shows: vec![],
                });
                ordinal += 1;
            }
            "ET" => {
                numbers(a, 0)?;
                let b = current.take().ok_or_else(invalid)?;
                if !b.text.is_empty() {
                    program.blocks.push(b);
                }
            }
            "q" => {
                numbers(a, 0)?;
                require(stack.len() < 64)?;
                stack.push(has_font);
            }
            "Q" => {
                numbers(a, 0)?;
                has_font = stack.pop().ok_or_else(invalid)?;
            }
            "Tf" => {
                require(a.len() == 2)?;
                let name = a[0].as_name().map_err(|_| invalid())?;
                require(number(&a[1])? > 0.0)?;
                let index = fonts.select(name)?;
                op.operands[0] = Object::Name(format!("F{index}").into_bytes());
                has_font = Some(index);
            }
            "Tj" | "TJ" | "'" | "\"" => {
                let font = has_font.ok_or_else(invalid)?;
                let block = current.as_mut().ok_or_else(invalid)?;
                match op.operator.as_str() {
                    "TJ" => {
                        require(a.len() == 1)?;
                        let arr = a[0].as_array().map_err(|_| invalid())?;
                        require(arr.len() <= 4096)?;
                        for item in arr {
                            match item {
                                Object::String(_, _) => text(item, block, budget, fonts, font)?,
                                _ => {
                                    number(item)?;
                                }
                            }
                        }
                    }
                    "\"" => {
                        require(a.len() == 3)?;
                        number(&a[0])?;
                        number(&a[1])?;
                        text(&a[2], block, budget, fonts, font)?;
                        program
                            .operations
                            .push(Operation::new("Tw", vec![a[0].clone()]));
                        program
                            .operations
                            .push(Operation::new("Tc", vec![a[1].clone()]));
                        program.operations.push(Operation::new("T*", vec![]));
                        op = Operation::new("Tj", vec![a[2].clone()]);
                    }
                    _ => {
                        require(a.len() == 1)?;
                        text(&a[0], block, budget, fonts, font)?;
                        if op.operator == "'" {
                            program.operations.push(Operation::new("T*", vec![]));
                            op.operator = "Tj".into();
                        }
                    }
                }
                block.shows.push(program.operations.len());
            }
            "Tm" | "Td" | "TD" | "T*" => {
                require(current.is_some())?;
                numbers(
                    a,
                    match op.operator.as_str() {
                        "Tm" => 6,
                        "T*" => 0,
                        _ => 2,
                    },
                )?;
            }
            "Tc" | "Tw" | "TL" | "Ts" => numbers(a, 1)?,
            "Tz" => {
                numbers(a, 1)?;
                require(number(&a[0])? > 0.0)?;
            }
            "Tr" => {
                numbers(a, 1)?;
                require(number(&a[0])? == 0.0)?;
            }
            "cm" => numbers(a, 6)?,
            "g" | "G" | "rg" | "RG" | "k" | "K" => {
                numbers(
                    a,
                    match op.operator.as_str() {
                        "g" | "G" => 1,
                        "rg" | "RG" => 3,
                        _ => 4,
                    },
                )?;
                for n in a {
                    require((0.0..=1.0).contains(&number(n)?))?;
                }
            }
            "w" | "M" => {
                numbers(a, 1)?;
                require(number(&a[0])? >= if op.operator == "M" { 1.0 } else { 0.0 })?;
            }
            "J" | "j" => {
                numbers(a, 1)?;
                require(matches!(a[0], Object::Integer(0..=2)))?;
            }
            "d" => {
                require(a.len() == 2)?;
                let arr = a[0].as_array().map_err(|_| invalid())?;
                require(arr.len() <= 4096)?;
                let mut sum = 0.0;
                for n in arr {
                    let n = number(n)?;
                    require(n >= 0.0)?;
                    sum += n;
                }
                require(arr.is_empty() || sum > 0.0)?;
                require(number(&a[1])? >= 0.0)?;
            }
            "m" | "l" | "c" | "v" | "y" | "re" | "h" | "S" | "s" | "f" | "F" | "f*" | "B"
            | "B*" | "b" | "b*" | "n" | "W" | "W*" => {
                require(current.is_none())?;
                numbers(
                    a,
                    match op.operator.as_str() {
                        "m" | "l" => 2,
                        "c" => 6,
                        "v" | "y" | "re" => 4,
                        _ => 0,
                    },
                )?;
            }
            _ => return Err(invalid()),
        }
        program.operations.push(op);
    }
    require(current.is_none() && stack.is_empty())?;
    Ok(program)
}
