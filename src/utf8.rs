//! SIMD-accelerated UTF-8 validation
//!
//! This module provides high-performance UTF-8 validation using the battle-tested
//! `simdutf8` crate, which is used by major projects like simd-json, polars, and arrow.
//!
//! Performance characteristics (from simdutf8 benchmarks):
//! - x86-64: Up to 23x faster than std on valid non-ASCII, 4x faster on ASCII
//! - aarch64: Up to 11x faster than std on valid non-ASCII, 4x faster on ASCII

/// Validate that the input is valid UTF-8
///
/// Returns true if the input is valid UTF-8, false otherwise.
/// Uses simdutf8's basic API which is optimized for the common case of valid UTF-8.
#[inline]
pub fn validate_utf8(data: &[u8]) -> bool {
    simdutf8::basic::from_utf8(data).is_ok()
}

/// Check if data is valid UTF-8 with incomplete sequence at the end
///
/// Returns:
/// - (true, n) if all complete sequences are valid, where n is the number of
///   trailing bytes that form an incomplete sequence (0-3 bytes)
/// - (false, 0) if there's an invalid UTF-8 sequence
///
/// This function is used for streaming UTF-8 validation where data may be
/// split across fragment boundaries in the middle of a multi-byte character.
pub fn validate_utf8_incomplete(data: &[u8]) -> (bool, usize) {
    if data.is_empty() {
        return (true, 0);
    }

    let len = data.len();
    let mut i = 0;

    while i < len {
        let b = data[i];

        if b < 0x80 {
            // ASCII byte
            i += 1;
        } else if b < 0xC0 {
            // Unexpected continuation byte at start of sequence
            return (false, 0);
        } else if b < 0xE0 {
            // 2-byte sequence: need 1 more byte
            if i + 1 >= len {
                // Incomplete - return how many bytes we have
                return (true, len - i);
            }
            let b1 = data[i + 1];
            if b1 & 0xC0 != 0x80 {
                return (false, 0);
            }
            // Check for overlong encoding
            if b < 0xC2 {
                return (false, 0);
            }
            i += 2;
        } else if b < 0xF0 {
            // 3-byte sequence: need 2 more bytes
            if i + 2 >= len {
                // Incomplete - but first validate what we have
                if i + 1 < len {
                    let b1 = data[i + 1];
                    if b1 & 0xC0 != 0x80 {
                        return (false, 0);
                    }
                    // Check for overlong and surrogate
                    if b == 0xE0 && b1 < 0xA0 {
                        return (false, 0);
                    }
                    if b == 0xED && b1 >= 0xA0 {
                        return (false, 0); // Surrogate
                    }
                }
                return (true, len - i);
            }
            let b1 = data[i + 1];
            let b2 = data[i + 2];
            if (b1 & 0xC0 != 0x80) || (b2 & 0xC0 != 0x80) {
                return (false, 0);
            }
            // Check for overlong encoding and surrogate halves
            let cp = ((b as u32 & 0x0F) << 12) | ((b1 as u32 & 0x3F) << 6) | (b2 as u32 & 0x3F);
            if cp < 0x800 || (0xD800..=0xDFFF).contains(&cp) {
                return (false, 0);
            }
            i += 3;
        } else if b < 0xF5 {
            // 4-byte sequence: need 3 more bytes
            if i + 3 >= len {
                // Incomplete - but first validate what we have
                if i + 1 < len {
                    let b1 = data[i + 1];
                    if b1 & 0xC0 != 0x80 {
                        return (false, 0);
                    }
                    // Check for overlong and out of range
                    if b == 0xF0 && b1 < 0x90 {
                        return (false, 0);
                    }
                    if b == 0xF4 && b1 >= 0x90 {
                        return (false, 0); // > U+10FFFF
                    }
                }
                if i + 2 < len {
                    let b2 = data[i + 2];
                    if b2 & 0xC0 != 0x80 {
                        return (false, 0);
                    }
                }
                return (true, len - i);
            }
            let b1 = data[i + 1];
            let b2 = data[i + 2];
            let b3 = data[i + 3];
            if (b1 & 0xC0 != 0x80) || (b2 & 0xC0 != 0x80) || (b3 & 0xC0 != 0x80) {
                return (false, 0);
            }
            // Check for overlong encoding and max codepoint
            let cp = ((b as u32 & 0x07) << 18)
                | ((b1 as u32 & 0x3F) << 12)
                | ((b2 as u32 & 0x3F) << 6)
                | (b3 as u32 & 0x3F);
            if !(0x10000..=0x10FFFF).contains(&cp) {
                return (false, 0);
            }
            i += 4;
        } else {
            // Invalid leading byte (>= 0xF5)
            return (false, 0);
        }
    }

    (true, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ascii() {
        assert!(validate_utf8(b"Hello, World!"));
        assert!(validate_utf8(b""));
        assert!(validate_utf8(b"0123456789"));
    }

    #[test]
    fn test_valid_utf8() {
        assert!(validate_utf8("Hello, 世界!".as_bytes()));
        assert!(validate_utf8("émoji: 🎉".as_bytes()));
        assert!(validate_utf8("Ñoño".as_bytes()));
        assert!(validate_utf8("日本語".as_bytes()));
    }

    #[test]
    fn test_invalid_utf8() {
        // Invalid continuation byte
        assert!(!validate_utf8(&[0xC0, 0x00]));

        // Overlong encoding
        assert!(!validate_utf8(&[0xC0, 0x80])); // Overlong NUL
        assert!(!validate_utf8(&[0xC1, 0xBF])); // Overlong

        // Invalid leading byte
        assert!(!validate_utf8(&[0xFF]));
        assert!(!validate_utf8(&[0xFE]));

        // Surrogate halves (invalid in UTF-8)
        assert!(!validate_utf8(&[0xED, 0xA0, 0x80])); // U+D800
        assert!(!validate_utf8(&[0xED, 0xBF, 0xBF])); // U+DFFF

        // Truncated sequences
        assert!(!validate_utf8(&[0xE0, 0x80])); // Missing byte
        assert!(!validate_utf8(&[0xF0, 0x80, 0x80])); // Missing byte

        // Invalid continuation
        assert!(!validate_utf8(&[0xE0, 0x80, 0x00]));
    }

    #[test]
    fn test_validate_utf8_incomplete() {
        // Complete ASCII
        let (valid, incomplete) = validate_utf8_incomplete(b"hello");
        assert!(valid);
        assert_eq!(incomplete, 0);

        // Incomplete 2-byte sequence
        let (valid, incomplete) = validate_utf8_incomplete(&[0xC2]);
        assert!(valid);
        assert_eq!(incomplete, 1);

        // Incomplete 3-byte sequence
        let (valid, incomplete) = validate_utf8_incomplete(&[0xE4, 0xB8]);
        assert!(valid);
        assert_eq!(incomplete, 2);

        // Complete followed by incomplete
        let mut data = b"hi".to_vec();
        data.push(0xE4);
        data.push(0xB8);
        let (valid, incomplete) = validate_utf8_incomplete(&data);
        assert!(valid);
        assert_eq!(incomplete, 2);
    }

    #[test]
    fn test_long_utf8() {
        // Test with longer strings to exercise SIMD paths
        let long_ascii = "a".repeat(1000);
        assert!(validate_utf8(long_ascii.as_bytes()));

        let long_unicode = "日本語".repeat(100);
        assert!(validate_utf8(long_unicode.as_bytes()));

        let mixed = format!("{}日本語{}", "a".repeat(100), "b".repeat(100));
        assert!(validate_utf8(mixed.as_bytes()));
    }
}
