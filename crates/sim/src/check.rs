//! Structural checks a decoded save passes before it becomes a world (ADR 0033).
//!
//! Decoding proves the bytes have the right shape. These checks prove that the lengths and
//! indices inside agree, so a loaded world cannot index out of bounds later.

/// The name of the first failed check.
pub(crate) type Check = Result<(), &'static str>;

pub(crate) fn ensure(ok: bool, what: &'static str) -> Check {
    if ok { Ok(()) } else { Err(what) }
}

/// Every length is `n`.
pub(crate) fn lens(lens: &[usize], n: usize, what: &'static str) -> Check {
    ensure(lens.iter().all(|&len| len == n), what)
}

/// Every ID is below `n`.
pub(crate) fn below<'a>(
    ids: impl IntoIterator<Item = &'a u32>,
    n: usize,
    what: &'static str,
) -> Check {
    ensure(ids.into_iter().all(|&id| (id as usize) < n), what)
}

/// `start` holds compressed-row offsets for `n` items into `len` entries. It stays empty until
/// the first item is pushed.
pub(crate) fn csr(start: &[u32], n: usize, len: usize, what: &'static str) -> Check {
    if start.is_empty() {
        return ensure(n == 0 && len == 0, what);
    }
    ensure(
        start.len() == n + 1
            && start[0] == 0
            && start.windows(2).all(|w| w[0] <= w[1])
            && start[n] as usize == len,
        what,
    )
}

/// Every value is a finite number.
pub(crate) fn finite<'a>(values: impl IntoIterator<Item = &'a f32>, what: &'static str) -> Check {
    ensure(values.into_iter().all(|v| v.is_finite()), what)
}
