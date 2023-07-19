/*!
 # RC5 block-cipher

 Library implementation of the basic RC5 block cipher in Rust. RC5 is different
 from the classical ciphers (like AES) in the sense that allows to parametrize
 the algorithm and optimize both security and efficiency on different hardware.

 These parameters are:

 * `w`: word length in bytes
 * `r`: number of rounds
 * `b`: key length in bytes

 The selection of each of them should be preferably done by choosing standards
 from other use cases. For example the word length `w` could be any number of
 bytes but the recommendation for performance and security is that should be a
 power of 2, or even better, a power of 8. In that way one can use the hardware
 registers more efficiently, e.g. 32-bits or 64-bits registers, with
 vectorization possibilities (AVX on Intel or SVE on ARM).

 This RC5 implementation is designed only for the standard values of `w` (powers
 of 8) making use of the standard Rust types: u8, u16, u32, u64, u128.

 ## Example: encryption

 ```rust
 use rc5_cipher::encode;

 let key = vec![
     0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
     0x0E, 0x0F,
 ];
 let pt = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
 let ct = vec![0x2D, 0xDC, 0x14, 0x9B, 0xCF, 0x08, 0x8B, 0x9E];

 let res = encode::<u32, 26>(key, pt);
 assert_eq!(ct, res.unwrap());
 ```

 ## Example: decryption

 ```rust
 use rc5_cipher::decode;

 let key = vec![
     0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
     0x0E, 0x0F,
 ];
 let pt = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
 let ct = vec![0x2D, 0xDC, 0x14, 0x9B, 0xCF, 0x08, 0x8B, 0x9E];

 let res = decode::<u32, 26>(key, ct);
 assert_eq!(pt, res.unwrap());
 ```

 ## Bibliography

 - Rivest original paper: https://www.grc.com/r&d/rc5.pdf
 - C implementation and tests: https://tools.ietf.org/id/draft-krovetz-rc6-rc5-vectors-00.html#rfc.section.4
 - Haskell implementation: https://hackage.haskell.org/package/cipher-rc5-0.1.1.2/docs/src/Crypto-Cipher-RC5.html

*/

use crate::unsigned::Unsigned;
use std::convert::TryInto;

#[derive(PartialEq, Debug)]
pub enum Error {
    BadLength,
    ConversionError,
}

#[inline(always)]
#[allow(arithmetic_overflow)]
fn rotl<W: Unsigned>(a: W, b: W) -> W {
    let shl = (b & (W::BITS - 1.into())).rem(W::BITS);
    let shr = ((W::BITS).wrapping_sub(&(b & (W::BITS - 1.into())))).rem(W::BITS);
    (a << shl) | (a >> shr)
}

#[inline(always)]
#[allow(arithmetic_overflow)]
fn rotr<W: Unsigned>(a: W, b: W) -> W {
    let shr = (b & (W::BITS - 1.into())).rem(W::BITS);
    let shl = ((W::BITS).wrapping_sub(&(b & (W::BITS - 1.into())))).rem(W::BITS);
    (a >> shr) | (a << shl)
}

///
/// Encrypts a plaintext `pt` and returns a ciphertext `ct`.
/// The `pt` should have length `2 * w = 2 * bytes(W)`
///
/// `W`: is the data type. Currently supported: u8, u16, u32, u64, u128
/// `T`: is the key expansion length `T = 2 * (r + 1)` being `r` number of
/// rounds. `T` should be even.
///
/// Example:
///
/// ```rust
/// use rc5_cipher::encode;
///
/// let key = vec![0x00, 0x01, 0x02, 0x03];
/// let pt  = vec![0x00, 0x01];
/// let ct  = vec![0x21, 0x2A];
/// let res = encode::<u8, 26>(key, pt).unwrap();
///     
/// assert!(&ct[..] == &res[..]);
/// ```
///
#[allow(arithmetic_overflow)]
pub fn encode<W, const T: usize>(key: Vec<u8>, pt: Vec<u8>) -> Result<Vec<u8>, Error>
where
    W: Unsigned,
    for<'a> &'a [u8]: TryInto<W::Array>,
{
    if pt.len() != 2 * W::BYTES {
        return Err(Error::BadLength);
    }
    let a: W::Array = pt[0..W::BYTES]
        .try_into()
        .map_err(|_| Error::ConversionError)?;
    let b: W::Array = pt[W::BYTES..2 * W::BYTES]
        .try_into()
        .map_err(|_| Error::ConversionError)?;

    let a: W = W::from_le_bytes(a);
    let b: W = W::from_le_bytes(b);

    let [a, b] = encode_kernel::<W, T>(key, [a, b]);

    Ok([W::to_le_bytes(a).as_slice(), W::to_le_bytes(b).as_slice()].concat())
}

#[allow(arithmetic_overflow)]
pub fn encode_kernel<W, const T: usize>(key: Vec<u8>, pt: [W; 2]) -> [W; 2]
where
    W: Unsigned,
{
    let key_exp = expand_key::<W, T>(key);
    let r = T / 2 - 1;
    let mut a = pt[0].wrapping_add(&key_exp[0]);
    let mut b = pt[1].wrapping_add(&key_exp[1]);
    for i in 1..=r {
        a = rotl(a ^ b, b).wrapping_add(&key_exp[2 * i]);
        b = rotl(b ^ a, a).wrapping_add(&key_exp[2 * i + 1]);
    }
    [a, b]
}

///
/// Decrypts a ciphertext `ct` and returns a plaintext `pt`.
/// The `ct` should have length 2 * w = 2 * bytes(W)
///
/// `W`: is the data type. Currently supported: u8, u16, u32, u64, u128
/// `T`: is the key expansion length `T = 2 * (r + 1)` being r number of rounds.
/// `T` should be even.
///
/// Example:
///
/// ```rust
/// use rc5_cipher::decode;
///
/// let key = vec![0x00, 0x01, 0x02, 0x03];
/// let pt  = vec![0x00, 0x01];
/// let ct  = vec![0x21, 0x2A];
/// let res = decode::<u8, 26>(key, ct.clone()).unwrap();
///
/// assert!(&pt[..] == &res[..]);
/// ```
///
#[allow(arithmetic_overflow)]
pub fn decode<W, const T: usize>(key: Vec<u8>, ct: Vec<u8>) -> Result<Vec<u8>, Error>
where
    W: Unsigned,
    for<'a> &'a [u8]: TryInto<W::Array>,
{
    if ct.len() != 2 * W::BYTES {
        return Err(Error::BadLength);
    }
    let a: W::Array = ct[0..W::BYTES]
        .try_into()
        .map_err(|_| Error::ConversionError)?;
    let b: W::Array = ct[W::BYTES..2 * W::BYTES]
        .try_into()
        .map_err(|_| Error::ConversionError)?;

    let a: W = W::from_le_bytes(a);
    let b: W = W::from_le_bytes(b);

    let [a, b] = decode_kernel::<W, T>(key, [a, b]);

    Ok([W::to_le_bytes(a).as_slice(), W::to_le_bytes(b).as_slice()].concat())
}

#[allow(arithmetic_overflow)]
pub fn decode_kernel<W, const T: usize>(key: Vec<u8>, ct: [W; 2]) -> [W; 2]
where
    W: Unsigned,
{
    let key_exp = expand_key::<W, T>(key);
    let r = T / 2 - 1;
    let mut a = ct[0];
    let mut b = ct[1];
    for i in (1..=r).rev() {
        b = rotr(b.wrapping_sub(&key_exp[2 * i + 1]), a) ^ a;
        a = rotr(a.wrapping_sub(&key_exp[2 * i]), b) ^ b;
    }
    [a.wrapping_sub(&key_exp[0]), b.wrapping_sub(&key_exp[1])]
}

///
/// Expands `key` into and array of length `T` of type `W`
///
/// `W`: is the data type. Currently supported: u8, u16, u32, u64, u128
/// `T`: is the key expansion length `T = 2 * (r + 1)` being `r` number of
/// rounds. `T` should be even.
///
/// Example:
///
/// ```rust
/// use rc5_cipher::expand_key;
///
/// let key = vec![0x00, 0x01, 0x02, 0x03];
/// let key_exp = expand_key::<u32, 4>(key);
///
/// assert_eq!(
///     &key_exp[..],
///     [0xbc13a1cf, 0xfeda18e9, 0x39252ff2, 0x57a51ad8]
/// );
/// ```
///
#[allow(arithmetic_overflow)]
pub fn expand_key<W, const T: usize>(key: Vec<u8>) -> [W; T]
where
    W: Unsigned,
{
    let mut key_s = [0.into(); T];
    let b = key.len();

    // c = max(1, ceil(8*b/w))
    let c = (std::cmp::max(
        1,
        (8 * key.len() + (W::BITSU32 - 1) as usize) as u32 / W::BITSU32,
    )) as usize;

    // converting the secrey key from bytes to words
    let mut key_l: Vec<W> = vec![0.into(); c];
    let u = W::BYTES as usize;
    for i in (0..=(b - 1)).rev() {
        let ix = (i / u) as usize;
        key_l[ix] = (key_l[ix].wrapping_shl(8u32)).wrapping_add(&W::from(key[i]));
    }

    // initializing array S
    key_s[0] = W::P;
    for i in 1..=(T - 1) {
        key_s[i] = key_s[i - 1].wrapping_add(&W::Q);
    }

    // Mixing in the secret key
    let mut i = 0;
    let mut j = 0;
    let mut a: W = 0.into();
    let mut b = 0.into();
    for _k in 0..3 * std::cmp::max(c, T) {
        key_s[i] = rotl(key_s[i].wrapping_add(&a.wrapping_add(&b)), 3.into());
        a = key_s[i];
        key_l[j] = rotl(
            key_l[j].wrapping_add(&a.wrapping_add(&b)),
            a.wrapping_add(&b),
        );
        b = key_l[j];
        i = (i + 1) % T;
        j = (j + 1) % c;
    }
    key_s
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn expand_key_a() {
    //     let key = vec![0x00, 0x01, 0x02, 0x03];
    //     let key_exp = expand_key::<u32, 4>(key);

    //     assert_eq!(
    //         &key_exp[..],
    //         [0xbc13a1cf, 0xfeda18e9, 0x39252ff2, 0x57a51ad8]
    //     );
    // }

    #[test]
    fn encode_fails_short_message() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let res = encode::<u32, 26>(key, pt).unwrap_err();

        assert_eq!(Error::BadLength, res);
    }

    #[test]
    fn encode_fails_long_message() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let res = encode::<u32, 26>(key, pt).unwrap_err();

        assert_eq!(Error::BadLength, res);
    }

    #[test]
    fn decode_fails_short_ciphertext() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ct = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let res = decode::<u32, 26>(key, ct).unwrap_err();

        assert_eq!(Error::BadLength, res);
    }

    #[test]
    fn decode_fails_long_ciphertext() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ct = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x00];
        let res = decode::<u32, 26>(key, ct).unwrap_err();

        assert_eq!(Error::BadLength, res);
    }

    #[test]
    fn encode_a() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let ct = vec![0x2D, 0xDC, 0x14, 0x9B, 0xCF, 0x08, 0x8B, 0x9E];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_b() {
        let key = vec![
            0x2B, 0xD6, 0x45, 0x9F, 0x82, 0xC5, 0xB3, 0x00, 0x95, 0x2C, 0x49, 0x10, 0x48, 0x81,
            0xFF, 0x48,
        ];
        let pt = vec![0xEA, 0x02, 0x47, 0x14, 0xAD, 0x5C, 0x4D, 0x84];
        let ct = vec![0x11, 0xE4, 0x3B, 0x86, 0xD2, 0x31, 0xEA, 0x64];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_c() {
        let key = vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let pt = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let ct = vec![0x21, 0xA5, 0xDB, 0xEE, 0x15, 0x4B, 0x8F, 0x6D];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_d() {
        let key = vec![
            0x91, 0x5F, 0x46, 0x19, 0xBE, 0x41, 0xB2, 0x51, 0x63, 0x55, 0xA5, 0x01, 0x10, 0xA9,
            0xCE, 0x91,
        ];
        let pt = vec![0x21, 0xA5, 0xDB, 0xEE, 0x15, 0x4B, 0x8F, 0x6D];
        let ct = vec![0xF7, 0xC0, 0x13, 0xAC, 0x5B, 0x2B, 0x89, 0x52];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_e() {
        let key = vec![
            0x78, 0x33, 0x48, 0xE7, 0x5A, 0xEB, 0x0F, 0x2F, 0xD7, 0xB1, 0x69, 0xBB, 0x8D, 0xC1,
            0x67, 0x87,
        ];
        let pt = vec![0xF7, 0xC0, 0x13, 0xAC, 0x5B, 0x2B, 0x89, 0x52];
        let ct = vec![0x2F, 0x42, 0xB3, 0xB7, 0x03, 0x69, 0xFC, 0x92];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_f() {
        let key = vec![
            0xDC, 0x49, 0xDB, 0x13, 0x75, 0xA5, 0x58, 0x4F, 0x64, 0x85, 0xB4, 0x13, 0xB5, 0xF1,
            0x2B, 0xAF,
        ];
        let pt = vec![0x2F, 0x42, 0xB3, 0xB7, 0x03, 0x69, 0xFC, 0x92];
        let ct = vec![0x65, 0xC1, 0x78, 0xB2, 0x84, 0xD1, 0x97, 0xCC];
        let res = encode::<u32, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn decode_a() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x96, 0x95, 0x0D, 0xDA, 0x65, 0x4A, 0x3D, 0x62];
        let ct = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_b() {
        let key = vec![
            0x2B, 0xD6, 0x45, 0x9F, 0x82, 0xC5, 0xB3, 0x00, 0x95, 0x2C, 0x49, 0x10, 0x48, 0x81,
            0xFF, 0x48,
        ];
        let pt = vec![0x63, 0x8B, 0x3A, 0x5E, 0xF7, 0x2B, 0x66, 0x3F];
        let ct = vec![0xEA, 0x02, 0x47, 0x14, 0xAD, 0x5C, 0x4D, 0x84];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_c() {
        let key = vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let pt = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let ct = vec![0x21, 0xA5, 0xDB, 0xEE, 0x15, 0x4B, 0x8F, 0x6D];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_d() {
        let key = vec![
            0x91, 0x5F, 0x46, 0x19, 0xBE, 0x41, 0xB2, 0x51, 0x63, 0x55, 0xA5, 0x01, 0x10, 0xA9,
            0xCE, 0x91,
        ];
        let pt = vec![0x21, 0xA5, 0xDB, 0xEE, 0x15, 0x4B, 0x8F, 0x6D];
        let ct = vec![0xF7, 0xC0, 0x13, 0xAC, 0x5B, 0x2B, 0x89, 0x52];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_e() {
        let key = vec![
            0x78, 0x33, 0x48, 0xE7, 0x5A, 0xEB, 0x0F, 0x2F, 0xD7, 0xB1, 0x69, 0xBB, 0x8D, 0xC1,
            0x67, 0x87,
        ];
        let pt = vec![0xF7, 0xC0, 0x13, 0xAC, 0x5B, 0x2B, 0x89, 0x52];
        let ct = vec![0x2F, 0x42, 0xB3, 0xB7, 0x03, 0x69, 0xFC, 0x92];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_f() {
        let key = vec![
            0xDC, 0x49, 0xDB, 0x13, 0x75, 0xA5, 0x58, 0x4F, 0x64, 0x85, 0xB4, 0x13, 0xB5, 0xF1,
            0x2B, 0xAF,
        ];
        let pt = vec![0x2F, 0x42, 0xB3, 0xB7, 0x03, 0x69, 0xFC, 0x92];
        let ct = vec![0x65, 0xC1, 0x78, 0xB2, 0x84, 0xD1, 0x97, 0xCC];
        let res = decode::<u32, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    /* Test cases from https://tools.ietf.org/id/draft-krovetz-rc6-rc5-vectors-00.html#rfc.section.4 */

    #[test]
    fn encode_8_12_4() {
        let key = vec![0x00, 0x01, 0x02, 0x03];
        let pt = vec![0x00, 0x01];
        let ct = vec![0x21, 0x2A];
        let res = encode::<u8, 26>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_kernel_8_12_4() {
        let key = vec![0x00, 0x01, 0x02, 0x03];
        let pt = [0x00, 0x01];
        let ct = [0x21, 0x2A];
        let res = encode_kernel::<u8, 26>(key, pt);
        assert!(&ct[..] == &res[..]);
    }

    #[test]
    fn encode_16_16_8() {
        let key = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let pt = vec![0x00, 0x01, 0x02, 0x03];
        let ct = vec![0x23, 0xA8, 0xD7, 0x2E];
        let res = encode::<u16, 34>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_kernel_16_16_8() {
        let key = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let pt = [0x0100, 0x0302];
        let ct = [0xA823, 0x2ED7];
        let res = encode_kernel::<u16, 34>(key, pt);
        assert!(&ct[..] == &res[..]);
    }

    #[test]
    fn encode_32_20_16() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let ct = vec![0x2A, 0x0E, 0xDC, 0x0E, 0x94, 0x31, 0xFF, 0x73];
        let res = encode::<u32, 42>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_kernel_32_20_16() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = [0x03020100, 0x07060504];
        let ct = [0x0EDC0E2A, 0x73FF3194];
        let res = encode_kernel::<u32, 42>(key, pt);
        assert!(&ct[..] == &res[..]);
    }

    #[test]
    fn encode_64_24_24() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        ];
        let pt = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ct = vec![
            0xA4, 0x67, 0x72, 0x82, 0x0E, 0xDB, 0xCE, 0x02, 0x35, 0xAB, 0xEA, 0x32, 0xAE, 0x71,
            0x78, 0xDA,
        ];
        let res = encode::<u64, 50>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_kernel_64_24_24() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        ];
        let pt = [0x0706050403020100, 0x0F0E0D0C0B0A0908];
        let ct = [0x2CEDB0E827267A4, 0xDA7871AE32EAAB35];
        let res = encode_kernel::<u64, 50>(key, pt);
        assert!(&ct[..] == &res[..]);
    }

    #[test]
    fn encode_128_28_32() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let pt = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let ct = vec![
            0xEC, 0xA5, 0x91, 0x09, 0x21, 0xA4, 0xF4, 0xCF, 0xDD, 0x7A, 0xD7, 0xAD, 0x20, 0xA1,
            0xFC, 0xBA, 0x06, 0x8E, 0xC7, 0xA7, 0xCD, 0x75, 0x2D, 0x68, 0xFE, 0x91, 0x4B, 0x7F,
            0xE1, 0x80, 0xB4, 0x40,
        ];
        let res = encode::<u128, 58>(key, pt);
        assert_eq!(ct, res.unwrap());
    }

    #[test]
    fn encode_kernel_128_28_32() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let pt = [
            0x0F0E0D0C0B0A09080706050403020100,
            0x1F1E1D1C1B1A19181716151413121110,
        ];
        let ct = [
            0xBAFCA120ADD77ADDCFF4A4210991A5EC,
            0x40B480E17F4B91FE682D75CDA7C78E06,
        ];
        let res = encode_kernel::<u128, 58>(key, pt);
        assert!(&ct[..] == &res[..]);
    }

    #[test]
    fn decode_8_12_4() {
        let key = vec![0x00, 0x01, 0x02, 0x03];
        let pt = vec![0x00, 0x01];
        let ct = vec![0x21, 0x2A];
        let res = decode::<u8, 26>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_kernel_8_12_4() {
        let key = vec![0x00, 0x01, 0x02, 0x03];
        let pt = [0x00, 0x01];
        let ct = [0x21, 0x2A];
        let res = decode_kernel::<u8, 26>(key, ct);
        assert!(&pt[..] == &res[..]);
    }

    #[test]
    fn decode_16_16_8() {
        let key = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let pt = vec![0x00, 0x01, 0x02, 0x03];
        let ct = vec![0x23, 0xA8, 0xD7, 0x2E];
        let res = decode::<u16, 34>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_kernel_16_16_8() {
        let key = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let pt = [0x0100, 0x0302];
        let ct = [0xA823, 0x2ED7];
        let res = decode_kernel::<u16, 34>(key, ct);
        assert!(&pt[..] == &res[..]);
    }

    #[test]
    fn decode_32_20_16() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        let ct = vec![0x2A, 0x0E, 0xDC, 0x0E, 0x94, 0x31, 0xFF, 0x73];
        let res = decode::<u32, 42>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_kernel_32_20_16() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let pt = [0x03020100, 0x07060504];
        let ct = [0x0EDC0E2A, 0x73FF3194];
        let res = decode_kernel::<u32, 42>(key, ct);
        assert!(&pt[..] == &res[..]);
    }

    #[test]
    fn decode_64_24_24() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        ];
        let pt = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let ct = vec![
            0xA4, 0x67, 0x72, 0x82, 0x0E, 0xDB, 0xCE, 0x02, 0x35, 0xAB, 0xEA, 0x32, 0xAE, 0x71,
            0x78, 0xDA,
        ];
        let res = decode::<u64, 50>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_kernel_64_24_24() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        ];
        let pt = [0x0706050403020100, 0x0F0E0D0C0B0A0908];
        let ct = [0x02CEDB0E827267A4, 0xDA7871AE32EAAB35];
        let res = decode_kernel::<u64, 50>(key, ct);
        assert!(&pt[..] == &res[..]);
    }

    #[test]
    fn decode_128_28_32() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let pt = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let ct = vec![
            0xEC, 0xA5, 0x91, 0x09, 0x21, 0xA4, 0xF4, 0xCF, 0xDD, 0x7A, 0xD7, 0xAD, 0x20, 0xA1,
            0xFC, 0xBA, 0x06, 0x8E, 0xC7, 0xA7, 0xCD, 0x75, 0x2D, 0x68, 0xFE, 0x91, 0x4B, 0x7F,
            0xE1, 0x80, 0xB4, 0x40,
        ];
        let res = decode::<u128, 58>(key, ct);
        assert_eq!(pt, res.unwrap());
    }

    #[test]
    fn decode_kernel_128_28_32() {
        let key = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F,
        ];
        let pt = [
            0x0F0E0D0C0B0A09080706050403020100,
            0x1F1E1D1C1B1A19181716151413121110,
        ];
        let ct = [
            0xBAFCA120ADD77ADDCFF4A4210991A5EC,
            0x40B480E17F4B91FE682D75CDA7C78E06,
        ];
        let res = decode_kernel::<u128, 58>(key, ct);
        assert!(&pt[..] == &res[..]);
    }
}
