use std::iter;

pub struct LeveledColons {
    levels: Vec<Vec<u32>>,
}

impl LeveledColons {
    pub fn build(bytes: &[u8], nesting: usize) -> Self {
        let chars = StructuralChars::build(bytes);

        let mut levels: Vec<Vec<u32>> = iter::repeat(chars.colon).take(nesting).collect();
        let mut stack = Vec::new();

        for i in 0..chars.right_brace.len() {
            let mut left_mask = chars.left_brace[i];
            let mut right_mask = chars.right_brace[i];

            loop {
                let mut right_bit = bitwise::extract(right_mask);
                let mut left_bit = bitwise::extract(left_mask);

                while left_bit > 0 && (right_bit == 0 || left_bit < right_bit) {
                    stack.push((i, left_bit));
                    left_mask = bitwise::remove(left_mask);
                    left_bit = bitwise::extract(left_mask);
                }

                if right_bit > 0 {
                    let (j, m) = stack.pop().expect("unexpected empty stack");
                    left_bit = m;

                    let depth = stack.len();
                    if depth > 0 && depth <= nesting {
                        if i == j {
                            levels[depth - 1][i] &= !(right_bit - left_bit);
                        } else {
                            levels[depth - 1][j] &= left_bit - 1;
                            levels[depth - 1][i] &= !(right_bit - 1);

                            for k in (j + 1)..i {
                                levels[depth][k] = 0;
                            }
                        }
                    }
                }

                right_mask = bitwise::remove(right_mask);

                if right_bit == 0 {
                    break;
                }
            }
        }

        LeveledColons { levels }
    }
}

struct StructuralChars {
    quote: Vec<u32>,
    colon: Vec<u32>,
    left_brace: Vec<u32>,
    right_brace: Vec<u32>,
}

impl StructuralChars {
    fn build(bytes: &[u8]) -> Self {
        let literals = LiteralChars::build(bytes);

        let mut quote = literals.quote.clone();
        let mut carry = 0;

        for i in (0..quote.len()).rev() {
            let mut quote_word = quote[i];
            let backslash_word = literals.backslash[i];

            let msb = carry << 31;
            carry = quote_word & 1;
            quote_word = (quote_word >> 1 | msb) & backslash_word;

            let mut escapes = quote_word;
            let mut mask = 0;

            while escapes > 0 {
                mask = !mask & bitwise::smear(escapes);

                if (backslash_word & mask).count_ones() % 2 == 0 {
                    // If there is an odd number of consecutive backslash characters,
                    // then we have found a non-structural quote (i.e. one that has been
                    // escaped by a backslash).
                    quote_word &= !bitwise::extract(escapes);
                }

                escapes = bitwise::remove(escapes);
            }

            if i + 1 < quote.len() {
                quote[i + 1] &= !(quote_word >> 31);
            }

            quote[i] &= !(quote_word << 1);
        }

        StructuralChars {
            quote: quote,
            colon: literals.colon,
            left_brace: literals.left_brace,
            right_brace: literals.right_brace,
        }
    }
}

#[cfg(target_arch = "x86")]
use std::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

struct LiteralChars {
    backslash: Vec<u32>,
    quote: Vec<u32>,
    colon: Vec<u32>,
    left_brace: Vec<u32>,
    right_brace: Vec<u32>,
}

impl LiteralChars {
    fn build(bytes: &[u8]) -> Self {
        if is_x86_feature_detected!("avx2") {
            unsafe { Self::build_avx2(bytes) }
        } else {
            panic!("CPU doesn't support AVX2 instructions");
        }
    }

    #[target_feature(enable = "avx2")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe fn build_avx2(bytes: &[u8]) -> Self {
        let capacity = (bytes.len() as f32 / 32.0).ceil() as usize;

        let mut backslash = Vec::with_capacity(capacity);
        let mut quote = Vec::with_capacity(capacity);
        let mut colon = Vec::with_capacity(capacity);
        let mut left_brace = Vec::with_capacity(capacity);
        let mut right_brace = Vec::with_capacity(capacity);

        unsafe fn bitmap_compare_avx2(input: __m256i, ch: u8) -> u32 {
            let chars = _mm256_set1_epi8(ch as i8);
            let cmp = _mm256_cmpeq_epi8(input, chars);
            _mm256_movemask_epi8(cmp) as u32
        }

        for chunk in bytes.chunks(32) {
            let input = if chunk.len() == 32 {
                _mm256_loadu_si256(chunk.as_ptr() as *const _)
            } else {
                let mut last = [0u8; 32];
                last[..chunk.len()].copy_from_slice(&chunk);
                _mm256_loadu_si256(last.as_ptr() as *const _)
            };

            backslash.push(bitmap_compare_avx2(input, b'\\'));
            quote.push(bitmap_compare_avx2(input, b'"'));
            colon.push(bitmap_compare_avx2(input, b':'));
            left_brace.push(bitmap_compare_avx2(input, b'{'));
            right_brace.push(bitmap_compare_avx2(input, b'}'));
        }

        LiteralChars {
            backslash,
            quote,
            colon,
            left_brace,
            right_brace,
        }
    }
}

mod bitwise {
    pub fn remove(x: u32) -> u32 {
        if x > 0 {
            x & (x - 1)
        } else {
            0
        }
    }

    pub fn extract(x: u32) -> u32 {
        (x as i32)
            .checked_neg()
            .map(|nx| x & nx as u32)
            .unwrap_or(x)
    }

    pub fn smear(x: u32) -> u32 {
        if x > 0 {
            x ^ x - 1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_chars() {
        let json = r#"{"id":"Apn5Q_b","name":"Minhas \"Micro\" Brewery","attributes":{"BikeParking":"False"}}"#;
        let bitmaps = LiteralChars::build(&json.as_bytes());

        assert_eq!(
            bitmaps.backslash,
            [
                0b10000000000000000000000000000000,
                0b00000000000000000000000001000000,
                0b00000000000000000000000000000000,
            ]
        );

        assert_eq!(
            bitmaps.quote,
            [
                0b00000000101000010100000001010010,
                0b00100000000001010000000010000001,
                0b00000000000100000101000000000001,
            ]
        );

        assert_eq!(
            bitmaps.colon,
            [
                0b00000000010000000000000000100000,
                0b01000000000000000000000000000000,
                0b00000000000000000010000000000000,
            ]
        );

        assert_eq!(
            bitmaps.left_brace,
            [
                0b00000000000000000000000000000001,
                0b10000000000000000000000000000000,
                0b00000000000000000000000000000000,
            ]
        );

        assert_eq!(
            bitmaps.right_brace,
            [
                0b00000000000000000000000000000000,
                0b00000000000000000000000000000000,
                0b00000000011000000000000000000000,
            ]
        );
    }

    #[test]
    fn test_structural_chars() {
        let json = r#"{"id":"Apn5Q_b","name":"Minhas \"Micro\" Brewery","attributes":{"BusinessParking":"{\"garage\":false}"}}"#;
        let bitmaps = StructuralChars::build(&json.as_bytes());

        assert_eq!(
            bitmaps.quote,
            [
                0b00000000101000010100000001010010,
                0b00100000000001010000000000000000,
                0b00000000000001010000000000000001,
                0b00000000000000000000000000100000,
            ]
        );
    }
}
