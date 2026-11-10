//! Private bounded JPEG coefficient editing foundation; no PDF admission yet.
use crate::{redaction_content::invalid, EngineResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transform {
    Default,
    Rgb,
    YCbCr,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub width: u32,
    pub height: u32,
    pub components: u32,
    pub mcu_width: u32,
    pub mcu_height: u32,
    pub columns: u32,
    pub rows: u32,
    pub scans: u32,
    pub transformed: bool,
    pub coefficient_work: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub memory_bytes: usize,
    pub output_bytes: usize,
    pub coefficient_work: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 192 * 1024 * 1024,
            output_bytes: 64 * 1024 * 1024,
            coefficient_work: 512_000_000,
        }
    }
}
pub(crate) struct Encoded {
    pub bytes: Vec<u8>,
    pub header: Header,
    pub peak_memory: usize,
}
#[path = "redaction_jpeg_format.rs"]
mod format;

#[repr(C)]
struct Request {
    width: u32,
    height: u32,
    components: u32,
    hmax: u32,
    vmax: u32,
    transformed: u32,
    scans: u32,
    memory_limit: usize,
    output_limit: usize,
    work_limit: u64,
}
#[repr(C)]
struct NativeResult {
    data: *mut u8,
    length: usize,
    peak_memory: usize,
    retained_memory: usize,
}
unsafe extern "C" {
    fn folio_jpeg_rewrite(
        input: *const u8,
        length: usize,
        request: *const Request,
        mask: *const u8,
        mask_length: usize,
        result: *mut NativeResult,
    ) -> std::ffi::c_int;
    fn folio_jpeg_release(data: *mut u8);
}
struct OwnedNative(*mut u8);
impl Drop for OwnedNative {
    fn drop(&mut self) {
        // The pointer is either null or the one allocation transferred by the C bridge.
        unsafe { folio_jpeg_release(self.0) };
    }
}
pub(crate) fn inspect(
    bytes: &[u8],
    expected: (u32, u32, u32),
    transform: Transform,
) -> EngineResult<Header> {
    Ok(format::parse(bytes, expected, transform)?.header)
}
pub(crate) fn rewrite(
    bytes: &[u8],
    expected: (u32, u32, u32),
    transform: Transform,
    mask: &[u8],
    limits: Limits,
) -> EngineResult<Encoded> {
    let parsed = format::parse(bytes, expected, transform)?;
    let h = &parsed.header;
    let maximum = Limits::default();
    if mask.len() != (h.columns * h.rows) as usize
        || mask.iter().any(|v| *v > 1)
        || limits.memory_bytes == 0
        || limits.memory_bytes > maximum.memory_bytes
        || limits.output_bytes == 0
        || limits.output_bytes > maximum.output_bytes
        || limits.coefficient_work == 0
        || limits.coefficient_work > maximum.coefficient_work
        || h.coefficient_work > limits.coefficient_work
    {
        return Err(invalid());
    }
    let request = Request {
        width: h.width,
        height: h.height,
        components: h.components,
        hmax: h.mcu_width / 8,
        vmax: h.mcu_height / 8,
        transformed: u32::from(h.transformed),
        scans: h.scans,
        memory_limit: limits.memory_bytes,
        output_limit: limits.output_bytes,
        work_limit: limits.coefficient_work,
    };
    let mut result = NativeResult {
        data: std::ptr::null_mut(),
        length: 0,
        peak_memory: 0,
        retained_memory: 0,
    };
    // All buffers live throughout this synchronous call. C contains every longjmp;
    // no callback into Rust, retained input pointer, or foreign unwind is allowed.
    let status = unsafe {
        folio_jpeg_rewrite(
            bytes.as_ptr(),
            bytes.len(),
            &request,
            mask.as_ptr(),
            mask.len(),
            &mut result,
        )
    };
    let owned = OwnedNative(result.data);
    if status != 0
        || owned.0.is_null()
        || result.length == 0
        || result.length > limits.output_bytes
        || result.retained_memory != 0
        || result.peak_memory > limits.memory_bytes
    {
        return Err(invalid());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(result.length)
        .map_err(|_| invalid())?;
    // On success C transfers an initialized allocation of at least `length` bytes.
    output.extend_from_slice(unsafe { std::slice::from_raw_parts(owned.0, result.length) });
    drop(owned);
    let checked = format::parse(&output, expected, transform)?;
    if !parsed.same_encoding(&checked) {
        return Err(invalid());
    }
    Ok(Encoded {
        bytes: output,
        header: checked.header,
        peak_memory: result.peak_memory,
    })
}

#[cfg(test)]
#[path = "redaction_jpeg_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "redaction_jpeg_decoder_tests.rs"]
mod decoder_tests;
