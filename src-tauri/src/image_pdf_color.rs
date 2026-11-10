//! Source color descriptions for image XObjects; alpha never passes through them.
use super::{compressed, invalid};
use crate::EngineResult;
use lopdf::{dictionary, Object};
use std::collections::BTreeSet;

pub(super) struct Description {
    pub space: Object,
    pub intent: Option<&'static str>,
}
fn device(components: u8) -> Object {
    Object::Name(match components {
        1 => b"DeviceGray".to_vec(),
        3 => b"DeviceRGB".to_vec(),
        4 => b"DeviceCMYK".to_vec(),
        _ => unreachable!(),
    })
}
fn intent(value: u32) -> EngineResult<&'static str> {
    match value {
        0 => Ok("Perceptual"),
        1 => Ok("RelativeColorimetric"),
        2 => Ok("Saturation"),
        3 => Ok("AbsoluteColorimetric"),
        _ => Err(invalid("Invalid image rendering intent.")),
    }
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("bounded metadata"),
    )
}

pub(super) fn icc_or_device(profile: Option<&[u8]>, components: u8) -> EngineResult<Description> {
    let Some(bytes) = profile else {
        return Ok(Description {
            space: device(components),
            intent: None,
        });
    };
    let malformed = || {
        invalid("The image has an unsupported or invalid ICC color profile. Convert it to sRGB before placing it.")
    };
    if bytes.len() < 132
        || bytes.len() > 16 * 1024 * 1024
        || u32_at(bytes, 0) as usize != bytes.len()
        || &bytes[36..40] != b"acsp"
        || !matches!(bytes[8], 2 | 4)
        || ![b"scnr", b"mntr", b"prtr", b"spac"].contains(&bytes[12..16].try_into().unwrap())
        || ![b"XYZ ", b"Lab "].contains(&bytes[20..24].try_into().unwrap())
    {
        return Err(malformed());
    }
    let expected = match components {
        1 => b"GRAY",
        3 => b"RGB ",
        4 => b"CMYK",
        _ => return Err(malformed()),
    };
    if &bytes[16..20] != expected {
        return Err(invalid(
            "The image's ICC profile does not match its color components.",
        ));
    }
    let count = u32_at(bytes, 128) as usize;
    if count == 0 || count > 4096 || 132 + 12 * count > bytes.len() {
        return Err(malformed());
    }
    let mut tags = BTreeSet::new();
    for index in 0..count {
        let at = 132 + 12 * index;
        let offset = u32_at(bytes, at + 4) as usize;
        let length = u32_at(bytes, at + 8) as usize;
        // Some otherwise usable profiles have unaligned tag offsets. Readers parse
        // bytes, not aligned pointers; retain them when all ranges/content validate.
        if offset < 132 + 12 * count
            || length < 8
            || offset
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
            || !tags.insert(bytes[at..at + 4].to_vec())
        {
            return Err(malformed());
        }
        let signature = &bytes[at..at + 4];
        let payload = &bytes[offset..offset + length];
        // The profile parser deliberately tolerates some missing/unknown values.
        // Require actual conversion payloads before trusting its parsed result.
        if [b"wtpt", b"rXYZ", b"gXYZ", b"bXYZ"].contains(&signature.try_into().unwrap())
            && (length < 20 || &payload[..4] != b"XYZ ")
        {
            return Err(malformed());
        }
        if [b"kTRC", b"rTRC", b"gTRC", b"bTRC"].contains(&signature.try_into().unwrap())
            && (length < 12 || ![b"curv", b"para"].contains(&payload[..4].try_into().unwrap()))
        {
            return Err(malformed());
        }
        if [b"A2B0", b"A2B1", b"A2B2", b"B2A0", b"B2A1", b"B2A2"]
            .contains(&signature.try_into().unwrap())
        {
            let forward = signature[0] == b'A';
            let expected_channels = if forward {
                [components, 3]
            } else {
                [3, components]
            };
            let valid_type = matches!(&payload[..4], b"mft1" | b"mft2")
                || (forward && &payload[..4] == b"mAB ")
                || (!forward && &payload[..4] == b"mBA ");
            if length < 12 || !valid_type || payload[8..10] != expected_channels {
                return Err(malformed());
            }
        }
    }
    let has = |tag: &[u8; 4]| tags.contains(tag.as_slice());
    let conversion = has(b"A2B0")
        || match components {
            1 => has(b"kTRC"),
            3 => [b"rXYZ", b"gXYZ", b"bXYZ", b"rTRC", b"gTRC", b"bTRC"]
                .into_iter()
                .all(has),
            _ => false,
        };
    if !has(b"wtpt") || !conversion {
        return Err(malformed());
    }
    let parsed = moxcms::ColorProfile::new_from_slice_with_options(
        bytes,
        moxcms::ParsingOptions {
            max_profile_size: 16 * 1024 * 1024,
            max_allowed_clut_size: 16 * 1024 * 1024,
            max_allowed_trc_size: 65536,
        },
    )
    .map_err(|_| malformed())?;
    if parsed.media_white_point.is_none()
        || (has(b"A2B0") && parsed.lut_a_to_b_perceptual.is_none())
        || (has(b"A2B1") && parsed.lut_a_to_b_colorimetric.is_none())
        || (has(b"A2B2") && parsed.lut_a_to_b_saturation.is_none())
        || (has(b"kTRC") && parsed.gray_trc.is_none())
        || (has(b"rTRC") && parsed.red_trc.is_none())
        || (has(b"gTRC") && parsed.green_trc.is_none())
        || (has(b"bTRC") && parsed.blue_trc.is_none())
    {
        return Err(malformed());
    }
    let rendering = intent(u32_at(bytes, 64))?;
    let stream = compressed(
        dictionary! {"N"=>components as i64,"Alternate"=>device(components)},
        bytes.to_vec(),
    )?;
    Ok(Description {
        space: Object::Array(vec![
            Object::Name(b"ICCBased".to_vec()),
            Object::Stream(stream),
        ]),
        intent: Some(rendering),
    })
}

pub(super) fn png(
    bytes: &[u8],
    profile: Option<&[u8]>,
    components: u8,
) -> EngineResult<Description> {
    let mut offset = 8;
    let mut gamma = None;
    let mut chromaticities = None;
    let mut srgb = None;
    let mut cicp_srgb = false;
    let mut declared_icc = false;
    while offset + 12 <= bytes.len() {
        let length = u32_at(bytes, offset) as usize;
        let name = &bytes[offset + 4..offset + 8];
        let data = bytes
            .get(offset + 8..offset + 8 + length)
            .ok_or_else(|| invalid("Incomplete PNG color metadata."))?;
        match name {
            b"iCCP" if declared_icc=>return Err(invalid("The PNG declares multiple ICC color profiles. Convert it to sRGB before placing it.")),
            b"iCCP"=>declared_icc=true,
            b"gAMA" if length==4=>gamma=Some(f64::from(u32_at(data,0))/100000.),
            b"cHRM" if length==32=>chromaticities=Some(std::array::from_fn::<_,8,_>(|i|f64::from(u32_at(data,4*i))/100000.)),
            b"sRGB" if length==1=>srgb=Some(intent(u32::from(data[0]))?),
            b"cICP" if data==[1,13,0,1]=>cicp_srgb=true,
            b"cICP"|b"mDCV"|b"cLLi"|b"cLLI"=>return Err(invalid("HDR image color metadata is not supported yet. Convert the image to sRGB PNG or JPEG first.")),
            b"gAMA"|b"cHRM"|b"sRGB"=>return Err(invalid("Invalid PNG color metadata.")),
            _=>{},
        }
        offset += length + 12;
    }
    if declared_icc && profile.is_none() {
        return Err(invalid(
            "The PNG declares an unusable ICC color profile. Convert it to sRGB before placing it.",
        ));
    }
    // Validate declared profiles even when a higher-priority cICP description wins.
    let icc = profile
        .map(|profile| icc_or_device(Some(profile), components))
        .transpose()?;
    if cicp_srgb {
        return Ok(Description {
            space: device(components),
            intent: Some("Perceptual"),
        });
    }
    if let Some(icc) = icc {
        return Ok(icc);
    }
    if let Some(rendering) = srgb {
        return Ok(Description {
            space: device(components),
            intent: Some(rendering),
        });
    }
    if gamma.is_none() && chromaticities.is_none() {
        return icc_or_device(None, components);
    }
    let gamma = 1. / gamma.unwrap_or(1. / 2.2);
    if !gamma.is_finite() || !(0.1..=10.).contains(&gamma) {
        return Err(invalid(
            "Unsupported PNG gamma. Convert the image to sRGB first.",
        ));
    }
    let xy = chromaticities.unwrap_or([0.3127, 0.3290, 0.64, 0.33, 0.30, 0.60, 0.15, 0.06]);
    calibration(xy)?;
    // PDFium's CalRGB image fast path ignores gamma/matrix. A matching ICC profile
    // keeps the original sample values and also renders correctly in that path.
    let bytes = calibrated_profile(xy, gamma, components)?;
    icc_or_device(Some(&bytes), components)
}

fn calibrated_profile(xy: [f64; 8], gamma: f64, components: u8) -> EngineResult<Vec<u8>> {
    use moxcms::{
        ColorProfile, LocalizableString, ProfileText, ToneReprCurve, XyY, WHITE_POINT_D50,
    };
    let source_white = XyY::new(xy[0], xy[1], 1.);
    let mut profile = if components == 1 {
        ColorProfile::new_gray_with_gamma(gamma as f32)
    } else {
        let mut profile = ColorProfile::new_srgb();
        profile.update_rgb_colorimetry_triplet(
            source_white,
            XyY::new(xy[2], xy[3], 1.).to_xyzd(),
            XyY::new(xy[4], xy[5], 1.).to_xyzd(),
            XyY::new(xy[6], xy[7], 1.).to_xyzd(),
        );
        let curve = ToneReprCurve::Parametric(vec![gamma as f32]);
        profile.red_trc = Some(curve.clone());
        profile.green_trc = Some(curve.clone());
        profile.blue_trc = Some(curve);
        profile
    };
    if components == 1 {
        profile.gray_trc = Some(ToneReprCurve::Parametric(vec![gamma as f32]));
    }
    profile.cicp = None;
    profile.media_white_point = Some(WHITE_POINT_D50.to_xyzd());
    profile.chromatic_adaptation = Some(moxcms::adaption_matrix_d(
        source_white.to_xyz(),
        WHITE_POINT_D50.to_xyz(),
    ));
    profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
        "en".into(),
        "US".into(),
        "Folio calibrated PNG".into(),
    )]));
    let mut bytes = profile
        .encode()
        .map_err(|e| invalid(format!("Cannot encode PNG color profile: {e}")))?;
    // moxcms0.8.1 writes wall-clock time even when creation_date_time is set. The
    // generated profile has a zero profile ID, so normalize only its date header.
    for (index, value) in [2000u16, 1, 1, 0, 0, 0].into_iter().enumerate() {
        bytes[24 + index * 2..26 + index * 2].copy_from_slice(&value.to_be_bytes());
    }
    Ok(bytes)
}
fn calibration(xy: [f64; 8]) -> EngineResult<([f64; 3], [f64; 9])> {
    let bad = || invalid("Invalid PNG chromaticities. Convert the image to sRGB first.");
    if xy
        .chunks_exact(2)
        .any(|p| !p.iter().all(|v| v.is_finite()) || p[0] < 0. || p[1] <= 0. || p[0] + p[1] > 1.)
    {
        return Err(bad());
    }
    let xyz = |x: f64, y: f64| [x / y, 1., (1. - x - y) / y];
    let white = xyz(xy[0], xy[1]);
    let columns = [xyz(xy[2], xy[3]), xyz(xy[4], xy[5]), xyz(xy[6], xy[7])];
    let determinant = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[1][0] * (a[0][1] * a[2][2] - a[0][2] * a[2][1])
            + a[2][0] * (a[0][1] * a[1][2] - a[0][2] * a[1][1])
    };
    let divisor = determinant(columns);
    if divisor.abs() < 1e-10 {
        return Err(bad());
    }
    let mut matrix = [0.; 9];
    for i in 0..3 {
        let mut replaced = columns;
        replaced[i] = white;
        let scale = determinant(replaced) / divisor;
        if !scale.is_finite() || scale <= 0. {
            return Err(bad());
        }
        for j in 0..3 {
            matrix[3 * i + j] = columns[i][j] * scale;
        }
    }
    Ok((white, matrix))
}
