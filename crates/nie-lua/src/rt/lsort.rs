//! `table.sort` of PUC-Rio Lua 5.2.4: a transcription of `auxsort` from `ltablib.c`.
//!
//! The algorithm is a Sedgewick quicksort (median of three, pivot parked at `u-1`, recursion on
//! the smaller half, loop on the larger). It is not stable and its sequence of comparator calls
//! depends on every detail, so the control flow below mirrors the C code line for line: a
//! script observing the comparator (a trace, a counter, an inconsistent order function) sees
//! exactly what the game's VM shows it.

/// Sorts `v` like `table.sort(t, cmp)` of Lua 5.2.4 would sort a table of `v.len()` elements.
///
/// `lt(a, b)` is the order function ("a sorts before b"); its errors abort the sort and are
/// returned as is. `invalid_order` builds the error raised when the comparator is inconsistent
/// (`"invalid order function for sorting"` in PUC).
///
/// # Errors
///
/// The error of `lt`, or `invalid_order()` when a scan runs past the partition bounds.
pub fn sort<T: Clone, E>(
    v: &mut [T],
    lt: &mut dyn FnMut(&T, &T) -> Result<bool, E>,
    invalid_order: impl Fn() -> E,
) -> Result<(), E> {
    let n = isize::try_from(v.len()).unwrap_or(isize::MAX);
    auxsort(v, 1, n, lt, &invalid_order)
}

/// 1-based Lua index to 0-based slice index.
const fn ix(i: isize) -> usize {
    (i - 1) as usize
}

fn auxsort<T: Clone, E>(
    v: &mut [T],
    mut l: isize,
    mut u: isize,
    lt: &mut dyn FnMut(&T, &T) -> Result<bool, E>,
    bad: &dyn Fn() -> E,
) -> Result<(), E> {
    while l < u {
        // sort a[l], a[(l+u)/2] and a[u]
        if lt(&v[ix(u)], &v[ix(l)])? {
            v.swap(ix(l), ix(u));
        }
        if u - l == 1 {
            break;
        }
        let mut i = (l + u) / 2;
        if lt(&v[ix(i)], &v[ix(l)])? {
            v.swap(ix(i), ix(l));
        } else if lt(&v[ix(u)], &v[ix(i)])? {
            v.swap(ix(i), ix(u));
        }
        if u - l == 2 {
            break;
        }
        let pivot = v[ix(i)].clone();
        v.swap(ix(i), ix(u - 1));
        // a[l] <= P == a[u-1] <= a[u]: only l+1..u-2 remain to be partitioned
        i = l;
        let mut j = u - 1;
        loop {
            // invariant: a[l..i] <= P <= a[j..u]
            loop {
                i += 1;
                if !lt(&v[ix(i)], &pivot)? {
                    break;
                }
                if i >= u {
                    return Err(bad());
                }
            }
            loop {
                j -= 1;
                if !lt(&pivot, &v[ix(j)])? {
                    break;
                }
                if j <= l {
                    return Err(bad());
                }
            }
            if j < i {
                break;
            }
            v.swap(ix(i), ix(j));
        }
        v.swap(ix(u - 1), ix(i));
        // a[l..i-1] <= a[i] == P <= a[i+1..u]: recurse into the smaller half, loop on the larger
        let (lo, hi);
        if i - l < u - i {
            lo = l;
            hi = i - 1;
            l = hi + 2;
        } else {
            lo = i + 1;
            hi = u;
            u = lo - 2;
        }
        auxsort(v, lo, hi, lt, bad)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mut v: Vec<i32>) -> (Vec<i32>, Vec<(i32, i32)>) {
        let mut trace = Vec::new();
        sort(
            &mut v,
            &mut |a, b| {
                trace.push((*a, *b));
                Ok::<_, &'static str>(a < b)
            },
            || "invalid order function for sorting",
        )
        .unwrap();
        (v, trace)
    }

    #[test]
    fn sorts_and_traces_small_inputs() {
        let (v, _) = run(vec![3, 1, 2]);
        assert_eq!(v, [1, 2, 3]);
        let (v, _) = run(vec![5, 4, 3, 2, 1, 0, 9, 8, 7, 6]);
        assert_eq!(v, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn trace_of_three_elements() {
        let (_, t) = run(vec![3, 1, 2]);
        assert_eq!(t, [(2, 3), (1, 2)]);
    }

    #[test]
    fn inconsistent_comparator_reports_invalid_order() {
        let mut v: Vec<i32> = (0..40).collect();
        let r = sort(&mut v, &mut |_, _| Ok::<_, &str>(true), || "invalid");
        assert_eq!(r, Err("invalid"));
    }

    #[test]
    fn comparator_error_propagates() {
        let mut v = vec![2, 1];
        let r = sort(&mut v, &mut |_, _| Err("boom"), || "invalid");
        assert_eq!(r, Err("boom"));
    }

    #[test]
    fn empty_and_single() {
        let mut v: Vec<i32> = Vec::new();
        assert!(sort(&mut v, &mut |a, b| Ok::<_, ()>(a < b), || ()).is_ok());
        let mut v = vec![1];
        assert!(sort(&mut v, &mut |a, b| Ok::<_, ()>(a < b), || ()).is_ok());
    }
}
