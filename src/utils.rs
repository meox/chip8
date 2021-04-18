use std::convert::TryFrom;

pub fn convert_to_bits(mut b: u8) -> [u8; 8] {
    let mut r: [u8; 8] = [0; 8];
    for x in 0..8 {
        let bit = b & 0x01;
        b >>= 1;
        r[7 - x] = bit;
    }

    r
}

pub fn convert_to_bcd(mut d: u16) -> [u8; 3] {
    let mut r: [u8; 3] = [0; 3];
    let mut i = 2;
    while d > 0 {
        let q = d % 10;
        d = (d - q) / 10;
        r[i] = u8::try_from(q).unwrap();
        if i == 0 {
            break;
        }
        i -= 1;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_tobits_tests() {
        assert_eq!([1, 0, 0, 0, 0, 0, 0, 0], convert_to_bits(0x80));
        assert_eq!([1, 1, 0, 0, 0, 0, 0, 0], convert_to_bits(0xC0));
        assert_eq!([1, 1, 1, 0, 0, 0, 0, 0], convert_to_bits(0xE0));
        assert_eq!([1, 1, 1, 1, 0, 0, 0, 0], convert_to_bits(0xF0));
        assert_eq!([1, 1, 1, 1, 1, 0, 0, 0], convert_to_bits(0xF8));
        assert_eq!([1, 1, 1, 1, 1, 1, 0, 0], convert_to_bits(0xFC));
        assert_eq!([1, 1, 1, 1, 1, 1, 1, 0], convert_to_bits(0xFE));
        assert_eq!([1, 1, 1, 1, 1, 1, 1, 1], convert_to_bits(0xFF));

        assert_eq!([1, 0, 1, 0, 1, 0, 1, 0], convert_to_bits(0xAA));
        assert_eq!([1, 1, 0, 0, 1, 0, 0, 1], convert_to_bits(0xC9));
    }

    #[test]
    fn conver_tobcd_tests() {
        assert_eq!([0, 0, 0], convert_to_bcd(0));
        assert_eq!([0, 0, 7], convert_to_bcd(7));
        assert_eq!([0, 2, 7], convert_to_bcd(27));
        assert_eq!([1, 2, 7], convert_to_bcd(127));
        assert_eq!([2, 5, 5], convert_to_bcd(255));
    }
}
