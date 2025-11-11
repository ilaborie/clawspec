use std::ops::{Range, RangeInclusive};

/// Expected status codes for HTTP requests.
///
/// Supports multiple ranges and individual status codes for flexible validation.
#[derive(Debug, Clone)]
pub struct ExpectedStatusCodes {
    ranges: Vec<StatusCodeRange>,
}

/// Represents a range of status codes (inclusive or exclusive).
#[derive(Debug, Clone)]
enum StatusCodeRange {
    Single(u16),
    Inclusive(RangeInclusive<u16>),
    Exclusive(Range<u16>),
}

impl ExpectedStatusCodes {
    /// Creates a new set of expected status codes with default range (200..500).
    pub fn new() -> Self {
        Self {
            ranges: vec![StatusCodeRange::Exclusive(200..500)],
        }
    }

    /// Adds a single status code as valid.
    pub fn add_single(mut self, status: u16) -> Self {
        self.ranges.push(StatusCodeRange::Single(status));
        self
    }

    /// Adds an inclusive range of status codes.
    pub fn add_inclusive_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Inclusive(range));
        self
    }

    /// Adds an exclusive range of status codes.
    pub fn add_exclusive_range(mut self, range: Range<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Exclusive(range));
        self
    }

    /// Creates expected status codes from a single inclusive range.
    ///
    /// # Panics
    ///
    /// Panics if the range contains invalid HTTP status codes (outside 100-599).
    pub fn from_inclusive_range(range: RangeInclusive<u16>) -> Self {
        assert!(
            *range.start() >= 100 && *range.start() <= 599,
            "HTTP status code range start must be between 100 and 599, got {}",
            range.start()
        );
        assert!(
            *range.end() >= 100 && *range.end() <= 599,
            "HTTP status code range end must be between 100 and 599, got {}",
            range.end()
        );
        assert!(
            range.start() <= range.end(),
            "HTTP status code range start ({}) must be less than or equal to end ({})",
            range.start(),
            range.end()
        );

        Self {
            ranges: vec![StatusCodeRange::Inclusive(range)],
        }
    }

    /// Creates expected status codes from a single exclusive range.
    ///
    /// # Panics
    ///
    /// Panics if the range contains invalid HTTP status codes (outside 100-599).
    pub fn from_exclusive_range(range: Range<u16>) -> Self {
        assert!(
            range.start >= 100 && range.start <= 599,
            "HTTP status code range start must be between 100 and 599, got {}",
            range.start
        );
        assert!(
            range.end >= 100 && range.end <= 600, // exclusive end can be 600
            "HTTP status code range end must be between 100 and 600 (exclusive), got {}",
            range.end
        );
        assert!(
            range.start < range.end,
            "HTTP status code range start ({}) must be less than end ({})",
            range.start,
            range.end
        );

        Self {
            ranges: vec![StatusCodeRange::Exclusive(range)],
        }
    }

    /// Creates expected status codes from a single status code.
    ///
    /// # Panics
    ///
    /// Panics if the status code is invalid (outside 100-599).
    pub fn from_single(status: u16) -> Self {
        assert!(
            (100..=599).contains(&status),
            "HTTP status code must be between 100 and 599, got {status}"
        );

        Self {
            ranges: vec![StatusCodeRange::Single(status)],
        }
    }

    /// Creates expected status codes from a single `http::StatusCode`.
    ///
    /// This method provides **compile-time validation** of status codes through the type system.
    /// Unlike the `u16` variants, this method does not perform runtime validation since
    /// `http::StatusCode` guarantees valid HTTP status codes at compile time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ExpectedStatusCodes;
    /// use http::StatusCode;
    ///
    /// let codes = ExpectedStatusCodes::from_status_code(StatusCode::OK);
    /// assert!(codes.contains(200));
    /// ```
    pub fn from_status_code(status: http::StatusCode) -> Self {
        // No runtime validation needed - http::StatusCode guarantees validity at compile time
        Self {
            ranges: vec![StatusCodeRange::Single(status.as_u16())],
        }
    }

    /// Creates expected status codes from an inclusive range of `http::StatusCode`.
    ///
    /// This method provides **compile-time validation** of status codes through the type system.
    /// Unlike the `u16` variants, this method does not perform runtime validation since
    /// `http::StatusCode` guarantees valid HTTP status codes at compile time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ExpectedStatusCodes;
    /// use http::StatusCode;
    ///
    /// let codes = ExpectedStatusCodes::from_status_code_range_inclusive(
    ///     StatusCode::OK..=StatusCode::NO_CONTENT
    /// );
    /// assert!(codes.contains(200));
    /// assert!(codes.contains(204));
    /// assert!(!codes.contains(205));
    /// ```
    pub fn from_status_code_range_inclusive(range: RangeInclusive<http::StatusCode>) -> Self {
        // No runtime validation needed - http::StatusCode guarantees validity at compile time
        let start = range.start().as_u16();
        let end = range.end().as_u16();
        Self {
            ranges: vec![StatusCodeRange::Inclusive(start..=end)],
        }
    }

    /// Creates expected status codes from an exclusive range of `http::StatusCode`.
    ///
    /// This method provides **compile-time validation** of status codes through the type system.
    /// Unlike the `u16` variants, this method does not perform runtime validation since
    /// `http::StatusCode` guarantees valid HTTP status codes at compile time.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ExpectedStatusCodes;
    /// use http::StatusCode;
    ///
    /// let codes = ExpectedStatusCodes::from_status_code_range_exclusive(
    ///     StatusCode::OK..StatusCode::PARTIAL_CONTENT
    /// );
    /// assert!(codes.contains(200));
    /// assert!(codes.contains(204));
    /// assert!(!codes.contains(206));
    /// ```
    pub fn from_status_code_range_exclusive(range: Range<http::StatusCode>) -> Self {
        // No runtime validation needed - http::StatusCode guarantees validity at compile time
        let start = range.start.as_u16();
        let end = range.end.as_u16();
        Self {
            ranges: vec![StatusCodeRange::Exclusive(start..end)],
        }
    }

    /// Checks if a status code is expected/valid.
    pub fn contains(&self, status: u16) -> bool {
        self.ranges.iter().any(|range| match range {
            StatusCodeRange::Single(s) => *s == status,
            StatusCodeRange::Inclusive(r) => r.contains(&status),
            StatusCodeRange::Exclusive(r) => r.contains(&status),
        })
    }

    /// Checks if an `http::StatusCode` is expected/valid.
    ///
    /// This is a convenience method that accepts `http::StatusCode` directly.
    ///
    /// # Example
    ///
    /// ```rust
    /// use clawspec_core::ExpectedStatusCodes;
    /// use http::StatusCode;
    ///
    /// let codes = ExpectedStatusCodes::from_status_code(StatusCode::OK);
    /// assert!(codes.contains_status_code(StatusCode::OK));
    /// assert!(!codes.contains_status_code(StatusCode::NOT_FOUND));
    /// ```
    pub fn contains_status_code(&self, status: http::StatusCode) -> bool {
        self.contains(status.as_u16())
    }

    /// Adds a single expected status code (method used by ApiCall).
    pub fn add_expected_status(mut self, status: u16) -> Self {
        self.ranges.push(StatusCodeRange::Single(status));
        self
    }

    /// Adds an expected inclusive range of status codes (method used by ApiCall).
    pub fn add_expected_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.ranges.push(StatusCodeRange::Inclusive(range));
        self
    }
}

impl Default for ExpectedStatusCodes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod status_code_tests {
    use super::*;
    use http::StatusCode;

    #[test]
    fn test_default_status_codes() {
        let codes = ExpectedStatusCodes::default();
        assert!(codes.contains(200));
        assert!(codes.contains(299));
        assert!(codes.contains(404));
        assert!(codes.contains(499));
        assert!(!codes.contains(500));
        assert!(!codes.contains(199));
    }

    #[test]
    fn test_single_status_code() {
        let codes = ExpectedStatusCodes::from_single(200);
        assert!(codes.contains(200));
        assert!(!codes.contains(201));
        assert!(!codes.contains(404));
    }

    #[test]
    fn test_inclusive_range() {
        let codes = ExpectedStatusCodes::from_inclusive_range(200..=204);
        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));
        assert!(!codes.contains(199));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_exclusive_range() {
        let codes = ExpectedStatusCodes::from_exclusive_range(200..205);
        assert!(codes.contains(200));
        assert!(codes.contains(202));
        assert!(codes.contains(204));
        assert!(!codes.contains(199));
        assert!(!codes.contains(205));
    }

    #[test]
    fn test_multiple_ranges() {
        let codes = ExpectedStatusCodes::default()
            .add_single(201)
            .add_inclusive_range(300..=304)
            .add_exclusive_range(400..405);

        // Default range (200..500)
        assert!(codes.contains(200));
        assert!(codes.contains(299));
        assert!(codes.contains(404));
        assert!(codes.contains(499));
        assert!(!codes.contains(500));

        // Added single status
        assert!(codes.contains(201));

        // Added inclusive range
        assert!(codes.contains(300));
        assert!(codes.contains(304));

        // Added exclusive range (405 is still contained due to default range 200..500)
        assert!(codes.contains(400));
        assert!(codes.contains(404));
        assert!(codes.contains(405)); // 405 is in the default range 200..500
    }

    #[test]
    fn test_status_code_variants() {
        let codes = ExpectedStatusCodes::from_status_code(StatusCode::OK);
        assert!(codes.contains_status_code(StatusCode::OK));
        assert!(!codes.contains_status_code(StatusCode::NOT_FOUND));

        let range_codes = ExpectedStatusCodes::from_status_code_range_inclusive(
            StatusCode::OK..=StatusCode::NO_CONTENT,
        );
        assert!(range_codes.contains_status_code(StatusCode::OK));
        assert!(range_codes.contains_status_code(StatusCode::CREATED));
        assert!(range_codes.contains_status_code(StatusCode::NO_CONTENT));
        assert!(!range_codes.contains_status_code(StatusCode::PARTIAL_CONTENT));

        let exclusive_codes = ExpectedStatusCodes::from_status_code_range_exclusive(
            StatusCode::OK..StatusCode::PARTIAL_CONTENT,
        );
        assert!(exclusive_codes.contains_status_code(StatusCode::OK));
        assert!(exclusive_codes.contains_status_code(StatusCode::NO_CONTENT));
        assert!(!exclusive_codes.contains_status_code(StatusCode::PARTIAL_CONTENT));
    }

    #[test]
    #[should_panic(expected = "HTTP status code must be between 100 and 599, got 99")]
    fn test_invalid_single_status_code_low() {
        ExpectedStatusCodes::from_single(99);
    }

    #[test]
    #[should_panic(expected = "HTTP status code must be between 100 and 599, got 600")]
    fn test_invalid_single_status_code_high() {
        ExpectedStatusCodes::from_single(600);
    }

    #[test]
    #[should_panic(expected = "HTTP status code range start must be between 100 and 599, got 99")]
    fn test_invalid_range_start_low() {
        ExpectedStatusCodes::from_inclusive_range(99..=200);
    }

    #[test]
    #[should_panic(expected = "HTTP status code range end must be between 100 and 599, got 600")]
    fn test_invalid_range_end_high() {
        ExpectedStatusCodes::from_inclusive_range(200..=600);
    }

    #[test]
    #[should_panic(
        expected = "HTTP status code range start (300) must be less than or equal to end (200)"
    )]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_invalid_range_order() {
        ExpectedStatusCodes::from_inclusive_range(300..=200);
    }

    #[test]
    #[should_panic(expected = "HTTP status code range start must be between 100 and 599, got 99")]
    fn test_invalid_exclusive_range_start() {
        ExpectedStatusCodes::from_exclusive_range(99..200);
    }

    #[test]
    #[should_panic(
        expected = "HTTP status code range end must be between 100 and 600 (exclusive), got 601"
    )]
    fn test_invalid_exclusive_range_end() {
        ExpectedStatusCodes::from_exclusive_range(200..601);
    }

    #[test]
    fn test_add_invalid_status() {
        // This should not panic because add_single doesn't validate
        let _codes = ExpectedStatusCodes::default().add_single(99);
    }

    #[test]
    fn test_add_invalid_range() {
        // This should not panic because add_inclusive_range doesn't validate
        let _codes = ExpectedStatusCodes::default().add_inclusive_range(99..=600);
    }
}
