//! Bounded marker framing before the native entropy decoder sees any input.
use super::{invalid, EngineResult, Header, Limits, Transform};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Component {
    id: u8,
    h: u8,
    v: u8,
    table: u8,
}
pub(super) struct Parsed {
    pub header: Header,
    components: Vec<Component>,
    tables: [Option<[u8; 64]>; 4],
}
impl Parsed {
    pub fn same_encoding(&self, other: &Self) -> bool {
        self.components == other.components
            && self.header.transformed == other.header.transformed
            && self
                .components
                .iter()
                .all(|c| self.tables[c.table as usize] == other.tables[c.table as usize])
    }
}
fn word(bytes: &[u8]) -> u32 {
    u16::from_be_bytes([bytes[0], bytes[1]]) as u32
}

pub(super) fn parse(
    bytes: &[u8],
    expected: (u32, u32, u32),
    transform: Transform,
) -> EngineResult<Parsed> {
    if bytes.len() > 64 * 1024 * 1024 || !bytes.starts_with(&[0xff, 0xd8]) {
        return Err(invalid());
    }
    let mut at = 2;
    let mut markers = 0;
    let mut scans = 0;
    let mut dimensions = None;
    let mut progressive = false;
    let mut components = Vec::<Component>::new();
    let mut tables = [None; 4];
    let mut huffman = [[false; 4]; 2];
    let mut bits = [[-1i8; 64]; 3];
    let mut jfif = false;
    let mut adobe = None;
    loop {
        markers += 1;
        if markers > 4096 || bytes.get(at) != Some(&0xff) {
            return Err(invalid());
        }
        while bytes.get(at) == Some(&0xff) {
            at += 1;
        }
        let marker = *bytes.get(at).ok_or_else(invalid)?;
        at += 1;
        if marker == 0xd9 {
            if at != bytes.len()
                || scans == 0
                || components.iter().enumerate().any(|(i, _)| bits[i][0] < 0)
            {
                return Err(invalid());
            }
            break;
        }
        let length = bytes.get(at..at + 2).ok_or_else(invalid).map(word)? as usize;
        if length < 2 {
            return Err(invalid());
        }
        let data = bytes.get(at + 2..at + length).ok_or_else(invalid)?;
        at += length;
        match marker {
            0xc0 | 0xc2 => {
                if dimensions.is_some() || data.len() < 6 || data[0] != 8 {
                    return Err(invalid());
                }
                let height = word(&data[1..3]);
                let width = word(&data[3..5]);
                let count = data[5] as usize;
                if width == 0
                    || height == 0
                    || width > 32768
                    || height > 32768
                    || u64::from(width) * u64::from(height) > 16_000_000
                    || !matches!(count, 1 | 3)
                    || data.len() != 6 + 3 * count
                    || (width, height, count as u32) != expected
                {
                    return Err(invalid());
                }
                for p in data[6..].chunks_exact(3) {
                    let c = Component {
                        id: p[0],
                        h: p[1] >> 4,
                        v: p[1] & 15,
                        table: p[2],
                    };
                    if !(1..=2).contains(&c.h)
                        || !(1..=2).contains(&c.v)
                        || c.table > 3
                        || components.iter().any(|v| v.id == c.id)
                    {
                        return Err(invalid());
                    }
                    components.push(c);
                }
                if components
                    .iter()
                    .map(|c| u32::from(c.h) * u32::from(c.v))
                    .sum::<u32>()
                    > 10
                    || (count == 1 && (components[0].h != 1 || components[0].v != 1))
                {
                    return Err(invalid());
                }
                dimensions = Some((width, height));
                progressive = marker == 0xc2;
            }
            0xdb => {
                let mut q = 0;
                while q < data.len() {
                    let id = data[q] as usize;
                    q += 1;
                    if id > 3 || q + 64 > data.len() {
                        return Err(invalid());
                    }
                    let table: [u8; 64] = data[q..q + 64].try_into().map_err(|_| invalid())?;
                    q += 64;
                    if table.contains(&0) || tables[id].is_some_and(|old| old != table) {
                        return Err(invalid());
                    }
                    tables[id] = Some(table);
                }
                if q == 0 {
                    return Err(invalid());
                }
            }
            0xc4 => {
                let mut p = 0;
                while p < data.len() {
                    if p + 17 > data.len() {
                        return Err(invalid());
                    }
                    let class = (data[p] >> 4) as usize;
                    let id = (data[p] & 15) as usize;
                    if class > 1 || id > 3 {
                        return Err(invalid());
                    }
                    let count = data[p + 1..p + 17]
                        .iter()
                        .map(|v| *v as usize)
                        .sum::<usize>();
                    if count == 0 || count > 256 || p + 17 + count > data.len() {
                        return Err(invalid());
                    }
                    // Libjpeg validates canonical code lengths and actual entropy use.
                    huffman[class][id] = true;
                    p += 17 + count;
                }
                if p == 0 {
                    return Err(invalid());
                }
            }
            0xdd => {
                if data.len() != 2 {
                    return Err(invalid());
                }
            }
            0xda => {
                scans += 1;
                if dimensions.is_none() || scans > 64 || data.is_empty() {
                    return Err(invalid());
                }
                let count = data[0] as usize;
                if count == 0 || count > components.len() || data.len() != 4 + 2 * count {
                    return Err(invalid());
                }
                let ss = data[1 + 2 * count] as usize;
                let se = data[2 + 2 * count] as usize;
                let ah = (data[3 + 2 * count] >> 4) as i8;
                let al = (data[3 + 2 * count] & 15) as i8;
                if ss > se
                    || se > 63
                    || ah > 13
                    || al > 13
                    || (!progressive && (ss != 0 || se != 63 || ah != 0 || al != 0))
                    || (progressive
                        && ((ss == 0 && se != 0)
                            || (ss > 0 && count != 1)
                            || (ah != 0 && ah != al + 1)))
                {
                    return Err(invalid());
                }
                let mut seen = [false; 3];
                for pair in data[1..1 + 2 * count].chunks_exact(2) {
                    let index = components
                        .iter()
                        .position(|c| c.id == pair[0])
                        .ok_or_else(invalid)?;
                    let dc = (pair[1] >> 4) as usize;
                    let ac = (pair[1] & 15) as usize;
                    if seen[index]
                        || dc > 3
                        || ac > 3
                        || tables[components[index].table as usize].is_none()
                        || ((ss == 0 && ah == 0) && !huffman[0][dc])
                        || (se > 0 && !huffman[1][ac])
                    {
                        return Err(invalid());
                    }
                    seen[index] = true;
                    for b in &mut bits[index][ss..=se] {
                        if (ah == 0 && *b != -1) || (ah != 0 && *b != ah) {
                            return Err(invalid());
                        }
                        *b = al;
                    }
                }
                // Stuffed FF bytes and restart markers belong to entropy data.
                // A real following marker is consumed by the outer loop.
                loop {
                    let value = *bytes.get(at).ok_or_else(invalid)?;
                    if value != 0xff {
                        at += 1;
                        continue;
                    }
                    let start = at;
                    at += 1;
                    while bytes.get(at) == Some(&0xff) {
                        at += 1;
                    }
                    let next = *bytes.get(at).ok_or_else(invalid)?;
                    if next == 0 {
                        if at != start + 1 {
                            return Err(invalid());
                        }
                        at += 1;
                    } else if (0xd0..=0xd7).contains(&next) {
                        at += 1;
                    } else {
                        at = start;
                        break;
                    }
                }
            }
            0xe0..=0xef | 0xfe => {
                if marker == 0xe2 && data.starts_with(b"ICC_PROFILE") {
                    return Err(invalid());
                }
                if marker == 0xe0 && data.starts_with(b"JFIF\0") {
                    // The native decoder fixes its interpretation at the first
                    // SOS. Later color hints must not redefine that decision.
                    if scans != 0
                        || jfif
                        || data.len() < 14
                        || data.len() != 14 + 3 * usize::from(data[12]) * usize::from(data[13])
                    {
                        return Err(invalid());
                    }
                    jfif = true;
                }
                if marker == 0xee && data.starts_with(b"Adobe") {
                    if scans != 0 || data.len() != 12 || data[11] > 1 || adobe.is_some() {
                        return Err(invalid());
                    }
                    adobe = Some(data[11]);
                }
            }
            _ => return Err(invalid()),
        }
    }
    let (width, height) = dimensions.ok_or_else(invalid)?;
    if jfif && adobe == Some(0) {
        return Err(invalid());
    }
    // PDF ColorTransform defaults to 1 for three components. Some readers
    // instead infer RGB from unmarked component IDs. Do not choose one reader's
    // interpretation for a stream whose selected coefficients would differ.
    if components.len() == 3
        && adobe.is_none()
        && !jfif
        && transform != Transform::Rgb
        && components.iter().map(|c| c.id).eq(b"RGB".iter().copied())
    {
        return Err(invalid());
    }
    let transformed = if components.len() == 1 {
        if adobe.is_some() {
            return Err(invalid());
        }
        false
    } else {
        match adobe {
            Some(0) => false,
            Some(1) => true,
            _ => transform != Transform::Rgb,
        }
    };
    let mw = 8 * u32::from(components.iter().map(|c| c.h).max().ok_or_else(invalid)?);
    let mh = 8 * u32::from(components.iter().map(|c| c.v).max().ok_or_else(invalid)?);
    let columns = width.div_ceil(mw);
    let rows = height.div_ceil(mh);
    let work = u64::from(columns)
        * u64::from(rows)
        * components
            .iter()
            .map(|c| u64::from(c.h) * u64::from(c.v))
            .sum::<u64>()
        * 64
        // Initialization, mask replacement and baseline output encoding each
        // receive another full padded-coefficient pass, beyond input scans.
        * (u64::from(scans)+3);
    if work > Limits::default().coefficient_work {
        return Err(invalid());
    }
    Ok(Parsed {
        header: Header {
            width,
            height,
            components: components.len() as u32,
            mcu_width: mw,
            mcu_height: mh,
            columns,
            rows,
            scans,
            transformed,
            coefficient_work: work,
        },
        components,
        tables,
    })
}
