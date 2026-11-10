//! Strict, image-specific 8-bit predictor decoding after bounded zlib inflation.
use crate::{
    redaction_content::{invalid, require},
    EngineResult,
};
use lopdf::{Dictionary, Object};

pub(super) struct Predictor {
    kind: i64,
    row: usize,
    colors: usize,
    height: usize,
}
impl Predictor {
    pub fn kind(&self) -> i64 {
        self.kind
    }
    pub fn new(dict: &Dictionary, width: u32, height: u32, colors: usize) -> EngineResult<Self> {
        // Image dimensions/components have already been admitted by Registry.
        let params = match dict.get(b"DecodeParms") {
            Err(_) | Ok(Object::Null) => None,
            Ok(Object::Dictionary(p)) => Some(p),
            _ => return Err(invalid()),
        };
        if let Some(p) = params {
            require(p.iter().all(|(k, _)| {
                [
                    b"Predictor".as_slice(),
                    b"Colors",
                    b"BitsPerComponent",
                    b"Columns",
                ]
                .contains(&k.as_slice())
            }))?;
        }
        let number = |key: &[u8], default: i64| -> EngineResult<i64> {
            match params.and_then(|p| p.get(key).ok()) {
                None => Ok(default),
                Some(Object::Integer(n)) => Ok(*n),
                _ => Err(invalid()),
            }
        };
        let kind = number(b"Predictor", 1)?;
        let components = number(b"Colors", 1)?;
        let columns = number(b"Columns", 1)?;
        require(
            matches!(kind, 1 | 2 | 10..=15)
                && (1..=4).contains(&components)
                && (1..=32768).contains(&columns)
                && number(b"BitsPerComponent", 8)? == 8,
        )?;
        if kind != 1 {
            require(
                dict.get(b"Filter").and_then(Object::as_name).ok() == Some(b"FlateDecode")
                    && components == colors as i64
                    && columns == i64::from(width),
            )?;
        }
        Ok(Self {
            kind,
            row: width as usize * colors,
            colors,
            height: height as usize,
        })
    }
    pub fn encoded_len(&self) -> usize {
        (self.row + usize::from(self.kind >= 10)) * self.height
    }
    pub fn decode(&self, mut bytes: Vec<u8>) -> EngineResult<Vec<u8>> {
        require(bytes.len() == self.encoded_len())?;
        if self.kind == 1 {
            return Ok(bytes);
        }
        let tagged = self.kind >= 10;
        for y in 0..self.height {
            let input = y * (self.row + usize::from(tagged));
            let output = y * self.row;
            let tag = if tagged { bytes[input] } else { 1 };
            require(tag <= 4)?;
            for x in 0..self.row {
                // Consume each input before writing its compacted output. The
                // destination is never ahead of unread input, even on row zero.
                let encoded = bytes[input + usize::from(tagged) + x];
                let left = if x >= self.colors {
                    bytes[output + x - self.colors]
                } else {
                    0
                };
                let up = if y > 0 {
                    bytes[output + x - self.row]
                } else {
                    0
                };
                let upper_left = if y > 0 && x >= self.colors {
                    bytes[output + x - self.row - self.colors]
                } else {
                    0
                };
                let prediction = match tag {
                    0 => 0,
                    1 => left,
                    2 => up,
                    3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                    4 => paeth(left, up, upper_left),
                    _ => return Err(invalid()),
                };
                bytes[output + x] = encoded.wrapping_add(prediction);
            }
        }
        bytes.truncate(self.row * self.height);
        Ok(bytes)
    }
}
fn paeth(left: u8, up: u8, upper_left: u8) -> u8 {
    let (a, b, c) = (i32::from(left), i32::from(up), i32::from(upper_left));
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        up
    } else {
        upper_left
    }
}
