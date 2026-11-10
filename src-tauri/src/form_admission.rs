//! Successful source admission only; the owner must keep its source immutable
//! and check current permissions before consulting this cache.
use std::sync::OnceLock;

#[derive(Default)]
pub(crate) struct Admission {
    succeeded: [OnceLock<()>; 2],
}

impl Admission {
    pub(crate) fn check<E>(
        &self,
        annotations: bool,
        validate: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        let slot = &self.succeeded[usize::from(annotations)];
        if slot.get().is_some() {
            return Ok(());
        }
        validate()?;
        // Worker requests are serialized. If this type is later shared, a
        // duplicate successful validation is harmless; never cache a failure.
        let _ = slot.set(());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn successful_admission_runs_once_per_mode() {
        let cache = Admission::default();
        let calls = Cell::new(0);
        for mode in [true, true, false, false, true] {
            cache.check(mode, || { calls.set(calls.get() + 1); Ok::<_, ()>(()) }).unwrap();
        }
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn permissive_success_does_not_authorize_strict_mode() {
        let cache = Admission::default();
        cache.check(true, || Ok::<_, &str>(())).unwrap();
        assert_eq!(cache.check(false, || Err("annotations")), Err("annotations"));
    }

    #[test]
    fn failures_are_retried_and_keep_their_error() {
        let cache = Admission::default();
        let calls = Cell::new(0);
        for expected in 1..=3 {
            let result = cache.check(true, || { calls.set(calls.get() + 1); Err(calls.get()) });
            assert_eq!(result, Err(expected));
        }
        cache.check(true, || Ok::<_, usize>(())).unwrap();
        assert_eq!(cache.check(true, || Err(4)), Ok(()));
    }

    #[test]
    fn another_source_starts_unvalidated() {
        let original = Admission::default();
        original.check(true, || Ok::<_, &str>(())).unwrap();
        let next = Admission::default();
        assert_eq!(next.check(true, || Err("changed source")), Err("changed source"));
    }

    #[test]
    fn panic_does_not_record_success() {
        let cache = Admission::default();
        assert!(std::panic::catch_unwind(|| cache.check::<()>(true, || panic!("validator"))).is_err());
        assert_eq!(cache.check(true, || Err("retry")), Err("retry"));
    }
}
