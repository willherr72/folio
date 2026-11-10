//! Opt-in aggregate allocation/work accounting. Ordinary loads have no scope.
use crate::{Document, Error, LoadOptions, Result};
use std::cell::RefCell;
use std::sync::Arc;
/// An opt-in predicate for original numeric object tokens during bounded loading.
pub type NumericTokenValidator = Arc<dyn Fn(&[u8], &crate::Object) -> bool + Send + Sync>;
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub object_bytes: usize,
    pub object_nodes: usize,
    pub xref_entries: usize,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            input_bytes: 64 * 1024 * 1024,
            decoded_bytes: 64 * 1024 * 1024,
            object_bytes: 128 * 1024 * 1024,
            object_nodes: 1_000_000,
            xref_entries: 200_000,
        }
    }
}
struct State {
    left: ResourceLimits,
    failed: Option<&'static str>,
    depth: usize,
    cancelled: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    numeric: Option<NumericTokenValidator>,
}
thread_local! {static STATE:RefCell<Option<State>>=const{RefCell::new(None)};}
fn error(reason: &'static str) -> Error {
    Error::InvalidStream(format!("Bounded PDF load refused: {reason}"))
}
pub(crate) fn active() -> bool {
    STATE.with(|s| s.borrow().is_some())
}
pub(crate) fn fail(reason: &'static str) -> Error {
    STATE.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.failed.get_or_insert(reason);
        }
    });
    error(reason)
}
pub(crate) fn check() -> Result<()> {
    let (failed, cancelled) = STATE.with(|s| s.borrow().as_ref()
        .map_or((None, None), |s| (s.failed, s.cancelled.clone())));
    if let Some(reason) = failed { return Err(error(reason)); }
    // Probes may unwind or attempt a nested bounded load. Never invoke them while
    // borrowing STATE, and latch cancellation just like an exhausted budget.
    if cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(fail("cancelled"));
    }
    Ok(())
}
pub(crate) fn numeric(token: &[u8], next: Option<u8>, value: &crate::Object) -> Result<()> {
    let validator=STATE.with(|s|s.borrow().as_ref().and_then(|s|s.numeric.clone()));
    let Some(validate)=validator else {return Ok(())};
    check()?;
    // Ordinary parsing may accept a numeric prefix of an indirect scalar. An
    // opted-in proof must receive a whole PDF token, never the prefix of 1e2
    // or 1.2.3. This does not change the ordinary parser's compatibility mode.
    if next.is_some_and(|b|!b" \t\n\r\0\x0c()<>[]{}/%".contains(&b)) {
        return Err(fail("numeric token boundary"));
    }
    // A predicate may unwind or try a nested bounded load. Never hold the
    // thread-local borrow while calling it, and latch refusal against recovery.
    if !validate(token,value) {
        return Err(fail("numeric token predicate"));
    }
    check()
}
fn charge(n: usize, reason: &'static str, select: impl FnOnce(&mut ResourceLimits) -> &mut usize) -> Result<()> {
    check()?;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(s) = s.as_mut() else { return Ok(()) };
        if let Some(reason) = s.failed {
            return Err(error(reason));
        }
        let left = select(&mut s.left);
        if n > *left {
            s.failed = Some(reason);
            return Err(error(reason));
        }
        *left -= n;
        Ok(())
    })
}
pub(crate) fn payload(n: usize) -> Result<()> {
    charge(n, "copied object bytes", |s| &mut s.object_bytes)
}
pub(crate) fn nodes(n: usize) -> Result<()> {
    charge(n, "parser object attempts", |s| &mut s.object_nodes)
}
pub(crate) fn entries(n: usize) -> Result<()> {
    charge(n, "xref/index entries", |s| &mut s.xref_entries)
}
pub(crate) fn decoded(n: usize) -> Result<()> {
    charge(n, "aggregate decoded bytes", |s| &mut s.decoded_bytes)
}
pub(crate) fn decode_limit(limit: Option<usize>) -> Option<usize> {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .map(|s| limit.unwrap_or(usize::MAX).min(s.left.decoded_bytes))
            .or(limit)
    })
}
pub(crate) struct Resolution(bool);
impl Drop for Resolution {
    fn drop(&mut self) {
        if self.0 {
            STATE.with(|s| {
                if let Some(s) = s.borrow_mut().as_mut() {
                    s.depth -= 1;
                }
            });
        }
    }
}
pub(crate) fn resolution() -> Result<Resolution> {
    check()?;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(s) = s.as_mut() else {
            return Ok(Resolution(false));
        };
        if s.depth >= 64 {
            s.failed = Some("indirect resolution depth");
            return Err(error("indirect resolution depth"));
        }
        s.depth += 1;
        Ok(Resolution(true))
    })
}
struct Scope;
impl Scope {
    #[cfg(test)]
    fn enter(limits: ResourceLimits) -> Result<Self> {
        Self::enter_controlled(limits, None, None)
    }
    fn enter_controlled(limits: ResourceLimits, cancelled: Option<Arc<dyn Fn() -> bool + Send + Sync>>, numeric: Option<NumericTokenValidator>) -> Result<Self> {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.is_some() {
                return Err(error("nested load scope"));
            }
            *s = Some(State {
                left: limits,
                failed: None,
                depth: 0,
                cancelled,
                numeric,
            });
            Ok(Self)
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        STATE.with(|s| *s.borrow_mut() = None);
    }
}
impl crate::content::Content<Vec<crate::content::Operation>> {
    /// Strictly decode already-decompressed content with aggregate logical limits.
    /// Returns remaining allowances for subsequent content streams. This is not
    /// a PDF graphics validator or an exact RSS bound. Inline images are refused
    /// by the controlled parser; ordinary content decoding remains unchanged.
    pub fn decode_strict_with_resource_control(
        buffer: &[u8],
        mut limits: ResourceLimits,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<(Self, ResourceLimits)> {
        limits.input_bytes = limits.input_bytes.checked_sub(buffer.len())
            .ok_or_else(|| error("input bytes"))?;
        let _scope = Scope::enter_controlled(limits, Some(cancelled), None)?;
        check()?;
        let result = Self::decode_strict(buffer);
        check()?;
        let content = result?;
        let remaining = STATE.with(|s| s.borrow().as_ref().expect("active bounded content scope").left);
        Ok((content, remaining))
    }
}
impl Document {
    /// Bounded loading with an additional predicate for original numeric object
    /// tokens, including decoded object streams. References, names, strings,
    /// xref offsets and raw stream payloads are not numeric object tokens.
    /// The predicate must be quick and must not start ordinary parsing. A
    /// refusal is latched; no tokens are retained after the callback returns.
    pub fn load_mem_with_numeric_resource_control(
        buffer: &[u8], limits: ResourceLimits,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
        validator: NumericTokenValidator,
    ) -> Result<(Document, ResourceLimits)> {
        Self::load_bounded(buffer, limits, Some(cancelled), Some(validator))
    }
    /// Controlled bounded loading; returns the remaining aggregate allowances.
    /// The probe must be a quick predicate, not perform ordinary PDF parsing.
    /// Cancellation is cooperative at accounting, token-scan and decoder checkpoints.
    pub fn load_mem_with_resource_control(
        buffer: &[u8],
        limits: ResourceLimits,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<(Document, ResourceLimits)> {
        Self::load_bounded(buffer, limits, Some(cancelled), None)
    }

    /// Strict reader-structure loading with aggregate logical resource bounds.
    /// Decoder scratch and allocator capacities are additional bounded overhead;
    /// this is not an exact RSS limit. Encrypted inputs and recovery are refused.
    pub fn load_mem_with_resource_limits(buffer: &[u8], limits: ResourceLimits) -> Result<Document> {
        Self::load_bounded(buffer, limits, None, None).map(|(document, _)| document)
    }
    fn load_bounded(buffer: &[u8], mut limits: ResourceLimits, cancelled: Option<Arc<dyn Fn() -> bool + Send + Sync>>, numeric: Option<NumericTokenValidator>) -> Result<(Document, ResourceLimits)> {
        if buffer.len() > limits.input_bytes {
            return Err(error("input bytes"));
        }
        limits.input_bytes -= buffer.len();
        let _scope = Scope::enter_controlled(limits, cancelled, numeric)?;
        check()?;
        let result = Self::load_mem_with_options(
            buffer,
            LoadOptions {
                strict: true,
                max_decompressed_size: Some(limits.decoded_bytes),
                ..Default::default()
            },
        );
        check()?;
        let document = result?;
        let remaining = STATE.with(|s| s.borrow().as_ref().expect("active bounded load scope").left);
        Ok((document, remaining))
    }
}
/// Reserve encoded token storage before the existing parser allocates it.
/// Encoded length is conservative for names/hex; nested literal bytes count
/// once per enclosing temporary vector, including eventual parent copies.
pub(crate) fn token(input: &[u8], kind: u8) -> Result<()> {
    if !active() || input.first() != Some(&kind) {
        return Ok(());
    }
    check()?;
    if kind == b'/' {
        let n = token_end(&input[1..], |b| b" \t\n\r\0\x0c()<>[]{}/%".contains(&b))?;
        return payload(n);
    }
    if kind == b'<' {
        if input.get(1) == Some(&b'<') {
            return Ok(());
        }
        let n = token_end(&input[1..], |b| b == b'>')?;
        return payload(n);
    }
    let mut depth = 1usize;
    let mut total = 0usize;
    let mut i = 1;
    let mut checkpoint = i;
    while i < input.len() {
        if i - checkpoint >= 16 * 1024 {
            check()?;
            checkpoint = i;
        }
        let b = input[i];
        total = total.checked_add(depth).ok_or_else(|| fail("literal byte overflow"))?;
        if b == b'\\' {
            i += 1;
            if i < input.len() {
                total = total.checked_add(depth).ok_or_else(|| fail("literal byte overflow"))?;
            }
        } else if b == b'(' {
            depth += 1;
            if depth > 101 {
                return Err(fail("literal nesting"));
            }
        } else if b == b')' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        i += 1;
    }
    payload(total)
}

fn token_end(input: &[u8], ends: impl Fn(u8) -> bool) -> Result<usize> {
    for (index, chunk) in input.chunks(16 * 1024).enumerate() {
        check()?;
        if let Some(offset) = chunk.iter().position(|byte| ends(*byte)) {
            return Ok(index * (16 * 1024) + offset);
        }
    }
    Ok(input.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_restores_on_nested_refusal_and_unwind() {
        {
            let _scope = Scope::enter(ResourceLimits::default()).unwrap();
            assert!(Scope::enter(ResourceLimits::default()).is_err());
            assert!(active());
            payload(1).unwrap();
        }
        assert!(!active());
        let _ = std::panic::catch_unwind(|| {
            let _scope = Scope::enter(ResourceLimits::default()).unwrap();
            panic!("test unwind")
        });
        assert!(!active());
    }
    #[test]
    fn discarded_attempts_and_depth_exhaustion_stay_sticky() {
        let mut l = ResourceLimits::default();
        l.object_nodes = 1;
        let _scope = Scope::enter(l).unwrap();
        nodes(1).unwrap();
        assert!(nodes(1).is_err());
        assert!(payload(0).is_err());
        assert!(check().is_err());
    }
}
