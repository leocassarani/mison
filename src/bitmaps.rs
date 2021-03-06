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

                            if depth < nesting {
                                for k in (j + 1)..i {
                                    levels[depth][k] = 0;
                                }
                            }
                        }
                    }
                } else {
                    break;
                }

                right_mask = bitwise::remove(right_mask);
            }
        }

        LeveledColons { levels }
    }

    pub fn positions(&self, level: usize) -> Vec<usize> {
        let mut pos = Vec::new();

        let length = if level < self.levels.len() {
            self.levels[level].len()
        } else {
            0
        };

        for i in 0..length {
            let mut colon_mask = self.levels[level][i];
            while colon_mask > 0 {
                let bit_mask = bitwise::extract(colon_mask);
                let offset = (i as u32) * 32 + (bit_mask - 1).count_ones();
                pos.push(offset as usize);
                colon_mask = bitwise::remove(colon_mask);
            }
        }

        pos
    }
}

struct StructuralChars {
    pub quote: Vec<u32>,
    pub colon: Vec<u32>,
    pub left_brace: Vec<u32>,
    pub right_brace: Vec<u32>,
}

impl StructuralChars {
    fn build(bytes: &[u8]) -> Self {
        let literals = LiteralChars::build(bytes);

        let mut quote = literals.quote.clone();
        let mut carry = 0;

        for i in (0..quote.len()).rev() {
            let quote_word = quote[i];
            let backslash_word = literals.backslash[i];

            // At any point, escaped_quotes is a bitmap that holds the quotes that we
            // intend to "turn off" at the end of this iteration of the for loop.
            // We initialise it with a 1 bit for every quote that is preceded by a
            // backslash, then for every candidate escaped quote, we look at the number
            // of consecutive backslashes that precede it to ensure we're not accidentally
            // escaping quotes that are actually structural. For example, in the JSON string
            // "foo\\\"bar" the second quote is escaped, but in "foo\\", it is structural.
            let mut escaped_quotes = (quote_word >> 1 | carry << 31) & backslash_word;
            carry = quote_word & 1;

            // We will use the escapes bitmap to drive our iteration by turning off one
            // bit at a time until there are none left.
            let mut escapes = escaped_quotes;
            let mut mask = 0;

            while escapes > 0 {
                mask = !mask & bitwise::smear(escapes);

                if (backslash_word & mask).count_ones() % 2 == 0 {
                    // If there is an even number of consecutive backslash characters,
                    // such as in the string "foo\\", then the following quote is still
                    // a structural quote, so we turn off the corresponding bit to avoid
                    // masking it later.
                    escaped_quotes &= !bitwise::extract(escapes);
                }

                escapes = bitwise::remove(escapes);
            }

            // If there is an escaped quote at the boundary between this word and the next,
            // then we reach over into the next vector element and turn off the MSB in the
            // structural quote bitmap.
            if i + 1 < quote.len() {
                quote[i + 1] &= !(escaped_quotes >> 31);
            }

            // We shifted quote_word right by 1 so it would line up with the backslash bitmap,
            // now we need to shift it back so it matches the position of the quotes in the JSON.
            quote[i] &= !(escaped_quotes << 1);
        }

        let string_mask = StringMask::build(&quote);
        let colon = string_mask.apply(&literals.colon);
        let left_brace = string_mask.apply(&literals.left_brace);
        let right_brace = string_mask.apply(&literals.right_brace);

        StructuralChars {
            quote,
            colon,
            left_brace,
            right_brace,
        }
    }
}

struct StringMask {
    mask: Vec<u32>,
}

impl StringMask {
    fn build(quote: &[u32]) -> Self {
        let mut mask = Vec::with_capacity(quote.len());
        let mut n = 0;

        for i in 0..quote.len() {
            let mut quote_word = quote[i];
            let mut mask_word = 0;

            while quote_word > 0 {
                mask_word ^= bitwise::smear(quote_word);
                quote_word = bitwise::remove(quote_word);
                n += 1;
            }

            if n % 2 == 1 {
                mask_word = !mask_word;
            }

            mask.push(mask_word);
        }

        StringMask { mask }
    }

    fn apply(&self, bitmap: &[u32]) -> Vec<u32> {
        bitmap.iter().zip(&self.mask).map(|(b, m)| b & !m).collect()
    }
}

#[cfg(target_arch = "x86")]
use std::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

struct LiteralChars {
    pub backslash: Vec<u32>,
    pub quote: Vec<u32>,
    pub colon: Vec<u32>,
    pub left_brace: Vec<u32>,
    pub right_brace: Vec<u32>,
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

    #[test]
    fn test_string_mask() {
        let quote = [
            0b00000000101000010100000001010010,
            0b00100000000001010000000000000000,
            0b00000000000001010000000000000001,
            0b00000000000000000000000000100000,
        ];

        let colon = [
            0b00000000000000001000000000100000,
            0b01000000000000000000000000000000,
            0b01000000000000100000000000000000,
            0b00000000000000000000000000000000,
        ];

        let string_mask = StringMask::build(&quote);

        assert_eq!(
            string_mask.mask,
            [
                0b11111111001111100111111110011100,
                0b00111111111110011111111111111111,
                0b11111111111110011111111111111110,
                0b00000000000000000000000000111111,
            ]
        );

        assert_eq!(
            string_mask.apply(&colon),
            [
                0b00000000000000001000000000100000,
                0b01000000000000000000000000000000,
                0b00000000000000100000000000000000,
                0b00000000000000000000000000000000,
            ]
        );
    }

    #[test]
    fn test_bitwise_remove() {
        assert_eq!(bitwise::remove(0), 0);
        assert_eq!(bitwise::remove(0b1), 0);
        assert_eq!(bitwise::remove(0b11101000), 0b11100000);
    }

    #[test]
    fn test_bitwise_extract() {
        assert_eq!(bitwise::extract(0), 0);
        assert_eq!(bitwise::extract(0b100), 0b100);
        assert_eq!(bitwise::extract(0b11101000), 0b00001000);
        assert_eq!(
            bitwise::extract(0b10000000000000000000000000000100),
            0b00000000000000000000000000000100
        );
        assert_eq!(
            bitwise::extract(0b10000000000000000000000000000000),
            0b10000000000000000000000000000000
        );
    }

    #[test]
    fn test_bitwise_smear() {
        assert_eq!(bitwise::smear(0), 0);
        assert_eq!(bitwise::smear(0b1), 0b1);
        assert_eq!(bitwise::smear(0b1000), 0b1111);
        assert_eq!(bitwise::smear(0b11101000), 0b00001111);
    }
}
