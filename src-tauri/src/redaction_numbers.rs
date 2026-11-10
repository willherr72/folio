//! Bounded correspondence between content numeric tokens and renderer values.
use crate::{
    redaction_content::{invalid, require},
    EngineResult,
};

pub(super) fn load_pdf(bytes: &[u8]) -> EngineResult<lopdf::Document> {
    lopdf::Document::load_mem_with_numeric_resource_control(
        bytes,
        lopdf::ResourceLimits::default(),
        std::sync::Arc::new(|| false),
        std::sync::Arc::new(dictionary_token),
    )
    .map(|(doc, _)| doc)
    .map_err(|_| invalid())
}

fn dictionary_token(token: &[u8], value: &lopdf::Object) -> bool {
    match value {
        lopdf::Object::Integer(n) => token.len() <= 64 && i32::try_from(*n).is_ok(),
        lopdf::Object::Real(n) => prove_token(token).is_ok_and(|proved| proved == *n),
        _ => false,
    }
}

pub(super) fn validate_document_numbers(doc: &lopdf::Document) -> EngineResult<()> {
    fn visit(value: &lopdf::Object, depth: usize, nodes: &mut usize) -> EngineResult<()> {
        require(depth <= 64 && *nodes < 1_000_000)?;
        *nodes += 1;
        match value {
            lopdf::Object::Integer(n) => {
                require(dictionary_token(n.to_string().as_bytes(), value))?
            }
            lopdf::Object::Real(n) => require(dictionary_token(n.to_string().as_bytes(), value))?,
            lopdf::Object::Array(values) => {
                for value in values {
                    visit(value, depth + 1, nodes)?;
                }
            }
            lopdf::Object::Dictionary(dict) => {
                for (_, value) in dict {
                    visit(value, depth + 1, nodes)?;
                }
            }
            lopdf::Object::Stream(stream) => {
                for (_, value) in &stream.dict {
                    visit(value, depth + 1, nodes)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    require(doc.objects.len() <= 100_000)?;
    let mut nodes = 0;
    for value in doc.objects.values() {
        visit(value, 0, &mut nodes)?;
    }
    for (_, value) in &doc.trailer {
        visit(value, 0, &mut nodes)?;
    }
    Ok(())
}

pub(super) fn prove_token(token: &[u8]) -> EngineResult<f32> {
    let value = interpret(token)?;
    // lopdf writes a real using Rust's Display spelling. The serialized token
    // must retain the same value in both readers, including when it is integral.
    require(interpret(value.to_string().as_bytes())? == value)?;
    Ok(value)
}

fn interpret(token: &[u8]) -> EngineResult<f32> {
    require(!token.is_empty() && token.len() <= 64)?;
    let text = std::str::from_utf8(token).map_err(|_| invalid())?;
    let negative = token[0] == b'-';
    let body = if matches!(token[0], b'+' | b'-') {
        &token[1..]
    } else {
        token
    };
    require(
        body.iter().any(u8::is_ascii_digit)
            && body.iter().all(|b| b.is_ascii_digit() || *b == b'.')
            && body.iter().filter(|b| **b == b'.').count() <= 1,
    )?;
    if !body.contains(&b'.') {
        let integer = text.parse::<i64>().map_err(|_| invalid())?;
        require((-1_000_000..=1_000_000).contains(&integer))?;
        // Every admitted integer is exact in binary32 and fits both readers'
        // integer accumulators. Leading zeros cannot overflow a prefix.
        return Ok(integer as f32);
    }
    let value = text.parse::<f32>().map_err(|_| invalid())?;
    require(value.is_finite() && value.abs() <= 1_000_000.)?;
    let point = token.iter().position(|b| *b == b'.').ok_or_else(invalid)?;
    let positive = value.abs();
    if point >= 10 {
        let point = body.iter().position(|b| *b == b'.').ok_or_else(invalid)?;
        let integer = body[..point]
            .iter()
            .try_fold(0_u32, |n, b| {
                n.checked_mul(10)
                    .and_then(|n| n.checked_add(u32::from(*b - b'0')))
            })
            .ok_or_else(invalid)?;
        require(integer <= 1_000_000)?;
        let mut numerator = [0_f32; 2];
        let mut denominator = 1_f32;
        for digit in &body[point + 1..] {
            let digit = f32::from(*digit - b'0');
            // Positive digit recurrence is monotone. Enclose ordinary and
            // contracted multiply/add without assuming compiler contraction.
            numerator = [
                (10. * numerator[0] + digit).min(numerator[0].mul_add(10., digit)),
                (10. * numerator[1] + digit).max(numerator[1].mul_add(10., digit)),
            ];
            denominator *= 10.;
        }
        require(numerator.iter().all(|n| n.is_finite()))?;
        for n in numerator {
            let interpreted = integer as f32 + n / denominator;
            require(interpreted.is_finite() && interpreted == positive)?;
        }
    } else {
        let integer_digits = body.iter().position(|b| *b == b'.').ok_or_else(invalid)?;
        let mut ordinal = 0_i32;
        let mut first = 0_i32;
        let mut count = 0_i32;
        let mut significand = 0_u32;
        for digit in body.iter().filter(|b| **b != b'.') {
            if count == 0 && *digit != b'0' {
                first = ordinal;
            }
            if (count > 0 || *digit != b'0') && count < 9 {
                significand = significand * 10 + u32::from(*digit - b'0');
                count += 1;
            }
            ordinal += 1;
        }
        if significand == 0 {
            require(value == 0.)?;
            return Ok(value);
        }
        let mut exponent = integer_digits as i32 - first - count;
        while exponent < -13 && significand % 10 == 0 {
            significand /= 10;
            exponent += 1;
        }
        // A nonzero decimal that rounds to zero takes MuPDF's underflow-to-one
        // fallback. Never authorize it as the zero that Rust/PDFium obtain.
        require(value != 0. && (-54..=13).contains(&exponent))?;
        let decimal = format!("{significand}e{exponent}");
        if exponent >= -13 {
            require(decimal.parse::<f32>().map_err(|_| invalid())? == positive)?;
        } else {
            // Independently bound a 32-significand-bit cached decimal power
            // and round-to-odd division. u bounds relative power rounding;
            // v bounds one unit in the intermediate's last place. No reader
            // table or implementation is copied into the product.
            let u = 2_f64.powi(-32);
            let v = 2_f64.powi(-31);
            let q = decimal.parse::<f64>().map_err(|_| invalid())?;
            let low = ((q.next_down() * (1. - v)).next_down() / (1. + u)).next_down();
            let high = ((q.next_up() * (1. + v)).next_up() / (1. - u)).next_up();
            require(low > 0. && low as f32 == positive && high as f32 == positive)?;
        }
    }
    require(!negative || value <= 0.)?;
    Ok(value)
}

#[cfg(test)]
#[path = "redaction_numbers_tests.rs"]
mod tests;
