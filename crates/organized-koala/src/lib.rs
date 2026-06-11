#![doc = include_str!("../README.md")]

/// Adds two numbers and returns the sum.
///
/// # Examples
///
/// ```
/// use organized_koala::add;
///
/// assert_eq!(add(2, 2), 4);
/// ```
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests;
