// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! TCVN3 (Vietnamese legacy encoding) decoder and encoder.
//!
//! TCVN3 is a multi-byte character encoding used for Vietnamese text.
//! It uses both single-byte and two-byte sequences to represent Vietnamese characters.

use crate::ascii::*;
use crate::handles::*;
use crate::variant::*;
use crate::DecoderResult;
use crate::EncoderResult;

// TCVN3 Unicode to byte sequence mapping
// Based on https://vietunicode.sourceforge.net/charset
const TCVN3_ENCODE_TABLE: &[(u16, &[u8])] = &[
    // Vietnamese uppercase vowels with tones
    (0x00C0, b"A\xB5"), // À -> Aµ
    (0x00C1, b"A\xB8"), // Á -> A¸ 
    (0x00C2, b"\xA2"),   // Â -> ¢
    (0x00C3, b"A\xB7"), // Ã -> A·
    (0x00C8, b"E\xCC"), // È -> EÌ
    (0x00C9, b"E\xD0"), // É -> EÐ
    (0x00CA, b"\xA3"),   // Ê -> £
    (0x00CC, b"I\xD7"), // Ì -> I×
    (0x00CD, b"I\xDD"), // Í -> IÝ
    (0x00D2, b"O\xDF"), // Ò -> Oß
    (0x00D3, b"O\xE3"), // Ó -> Oã
    (0x00D4, b"\xA4"),   // Ô -> ¤
    (0x00D5, b"O\xE2"), // Õ -> Oâ
    (0x00D9, b"U\xEF"), // Ù -> Uï
    (0x00DA, b"U\xF3"), // Ú -> Uó
    (0x00DD, b"Y\xFD"), // Ý -> Yý
    
    // Vietnamese lowercase vowels with tones
    (0x00E0, b"\xB5"),   // à -> µ
    (0x00E1, b"\xB8"),   // á -> ¸
    (0x00E2, b"\xA9"),   // â -> ©
    (0x00E3, b"\xB7"),   // ã -> ·
    (0x00E8, b"\xCC"),   // è -> Ì
    (0x00E9, b"\xD0"),   // é -> Ð
    (0x00EA, b"\xAA"),   // ê -> ª
    (0x00EC, b"\xD7"),   // ì -> ×
    (0x00ED, b"\xDD"),   // í -> Ý
    (0x00F2, b"\xDF"),   // ò -> ß
    (0x00F3, b"\xE3"),   // ó -> ã
    (0x00F4, b"\xAB"),   // ô -> «
    (0x00F5, b"\xE2"),   // õ -> â
    (0x00F9, b"\xEF"),   // ù -> ï
    (0x00FA, b"\xF3"),   // ú -> ó
    (0x00FD, b"\xFD"),   // ý -> ý
    
    // Special Vietnamese characters
    (0x0102, b"\xA1"),   // Ă -> ¡
    (0x0103, b"\xA8"),   // ă -> ¨
    (0x0110, b"\xA7"),   // Đ -> §
    (0x0111, b"\xAE"),   // đ -> ®
    (0x0128, b"I\xDC"), // Ĩ -> IÜ
    (0x0129, b"\xDC"),   // ĩ -> Ü
    (0x0168, b"U\xF2"), // Ũ -> Uò
    (0x0169, b"\xF2"),   // ũ -> ò
    (0x01A0, b"\xA5"),   // Ơ -> ¥
    (0x01A1, b"\xAC"),   // ơ -> ¬
    (0x01AF, b"\xA6"),   // Ư -> ¦
    (0x01B0, b"\xAD"),   // ư -> ­
];

// TCVN3 decode sequences to Unicode mapping
// Order matters: longer sequences must come first
const TCVN3_DECODE_TABLE: &[(&[u8], u16)] = &[
    // Two-byte sequences first
    (b"A\xB5", 0x00C0), // Aµ -> À
    (b"A\xB8", 0x00C1), // A¸ -> Á
    (b"A\xB7", 0x00C3), // A· -> Ã
    (b"E\xCC", 0x00C8), // EÌ -> È
    (b"E\xD0", 0x00C9), // EÐ -> É
    (b"I\xD7", 0x00CC), // I× -> Ì
    (b"I\xDD", 0x00CD), // IÝ -> Í
    (b"O\xDF", 0x00D2), // Oß -> Ò
    (b"O\xE3", 0x00D3), // Oã -> Ó
    (b"O\xE2", 0x00D5), // Oâ -> Õ
    (b"U\xEF", 0x00D9), // Uï -> Ù
    (b"U\xF3", 0x00DA), // Uó -> Ú
    (b"Y\xFD", 0x00DD), // Yý -> Ý
    (b"I\xDC", 0x0128), // IÜ -> Ĩ
    (b"U\xF2", 0x0168), // Uò -> Ũ
    
    // Single byte sequences
    (b"\xA1", 0x0102), // ¡ -> Ă
    (b"\xA2", 0x00C2), // ¢ -> Â
    (b"\xA3", 0x00CA), // £ -> Ê
    (b"\xA4", 0x00D4), // ¤ -> Ô
    (b"\xA5", 0x01A0), // ¥ -> Ơ
    (b"\xA6", 0x01AF), // ¦ -> Ư
    (b"\xA7", 0x0110), // § -> Đ
    (b"\xA8", 0x0103), // ¨ -> ă
    (b"\xA9", 0x00E2), // © -> â
    (b"\xAA", 0x00EA), // ª -> ê
    (b"\xAB", 0x00F4), // « -> ô
    (b"\xAC", 0x01A1), // ¬ -> ơ
    (b"\xAD", 0x01B0), // ­ -> ư
    (b"\xAE", 0x0111), // ® -> đ
    (b"\xB5", 0x00E0), // µ -> à
    (b"\xB7", 0x00E3), // · -> ã
    (b"\xB8", 0x00E1), // ¸ -> á
    (b"\xCC", 0x00E8), // Ì -> è
    (b"\xD0", 0x00E9), // Ð -> é
    (b"\xD7", 0x00EC), // × -> ì
    (b"\xDC", 0x0129), // Ü -> ĩ
    (b"\xDD", 0x00ED), // Ý -> í
    (b"\xDF", 0x00F2), // ß -> ò
    (b"\xE2", 0x00F5), // â -> õ
    (b"\xE3", 0x00F3), // ã -> ó
    (b"\xEF", 0x00F9), // ï -> ù
    (b"\xF2", 0x0169), // ò -> ũ
    (b"\xF3", 0x00FA), // ó -> ú
    (b"\xFD", 0x00FD), // ý -> ý
];

pub struct Tcvn3Decoder {
    /// Pending byte from previous iteration
    pending: Option<u8>,
}

impl Tcvn3Decoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::Tcvn3(Tcvn3Decoder {
            pending: None,
        })
    }

    pub fn in_neutral_state(&self) -> bool {
        self.pending.is_none()
    }

    decoder_functions!(
        {
            // if self.pending.is_some() {
            //     return self.decode_pending_to_utf8(dst, last);
            // }
        },
        {
            // if self.pending.is_some() {
            //     return self.decode_pending_to_utf16(dst, last);
            // }
        },
        {
            let mut src_pos = 0usize;
            let mut dst_pos = 0usize;
            'outer: loop {
                if src_pos >= src.len() {
                    return (DecoderResult::InputEmpty, src_pos, dst_pos);
                }
                if dst_pos >= dst.len() {
                    return (DecoderResult::OutputFull, src_pos, dst_pos);
                }

                let first_byte = src[src_pos];
                
                // Handle ASCII
                if first_byte < 0x80 {
                    dst[dst_pos] = first_byte;
                    src_pos += 1;
                    dst_pos += 1;
                    continue;
                }

                // Check for two-byte sequences
                if src_pos + 1 < src.len() {
                    let two_bytes = &src[src_pos..src_pos + 2];
                    for &(pattern, unicode) in TCVN3_DECODE_TABLE {
                        if pattern.len() == 2 && two_bytes == pattern {
                            if unicode <= 0x7F {
                                dst[dst_pos] = unicode as u8;
                                dst_pos += 1;
                            } else {
                                // Need to handle non-ASCII in UTF-8 context
                                let ch = unsafe { char::from_u32_unchecked(unicode as u32) };
                                let utf8_bytes = ch.to_string().as_bytes().to_vec();
                                if dst_pos + utf8_bytes.len() > dst.len() {
                                    return (DecoderResult::OutputFull, src_pos, dst_pos);
                                }
                                dst[dst_pos..dst_pos + utf8_bytes.len()].copy_from_slice(&utf8_bytes);
                                dst_pos += utf8_bytes.len();
                            }
                            src_pos += 2;
                            continue 'outer;
                        }
                    }
                }

                // Check for single-byte sequences
                let one_byte = &src[src_pos..src_pos + 1];
                for &(pattern, unicode) in TCVN3_DECODE_TABLE {
                    if pattern.len() == 1 && one_byte == pattern {
                        if unicode <= 0x7F {
                            dst[dst_pos] = unicode as u8;
                            dst_pos += 1;
                        } else {
                            // Need to handle non-ASCII in UTF-8 context
                            let ch = unsafe { char::from_u32_unchecked(unicode as u32) };
                            let utf8_bytes = ch.to_string().as_bytes().to_vec();
                            if dst_pos + utf8_bytes.len() > dst.len() {
                                return (DecoderResult::OutputFull, src_pos, dst_pos);
                            }
                            dst[dst_pos..dst_pos + utf8_bytes.len()].copy_from_slice(&utf8_bytes);
                            dst_pos += utf8_bytes.len();
                        }
                        src_pos += 1;
                        continue 'outer;
                    }
                }

                // Unknown byte - use replacement
                if last || src_pos + 1 < src.len() {
                    dst[dst_pos] = b'?';
                    src_pos += 1;
                    dst_pos += 1;
                } else {
                    // Need more input
                    self.pending = Some(first_byte);
                    return (DecoderResult::InputEmpty, src_pos, dst_pos);
                }
            }
        },
        {},
        {
            // UTF-16 decode implementation similar to above
            // but writing u16 values instead of u8
            tcvn3_decode_to_utf16_impl(self, src, dst, last)
        },
        {
            // Handle ASCII and return
            ascii_to_ascii(src, dst)
        }
    );
}

fn tcvn3_decode_to_utf16_impl(decoder: &mut Tcvn3Decoder, src: &[u8], dst: &mut [u16], last: bool) -> (DecoderResult, usize, usize) {
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;
    
    'outer: loop {
        if src_pos >= src.len() {
            return (DecoderResult::InputEmpty, src_pos, dst_pos);
        }
        if dst_pos >= dst.len() {
            return (DecoderResult::OutputFull, src_pos, dst_pos);
        }

        let first_byte = src[src_pos];
        
        // Handle ASCII
        if first_byte < 0x80 {
            dst[dst_pos] = first_byte as u16;
            src_pos += 1;
            dst_pos += 1;
            continue;
        }

        // Check for two-byte sequences
        if src_pos + 1 < src.len() {
            let two_bytes = &src[src_pos..src_pos + 2];
            for &(pattern, unicode) in TCVN3_DECODE_TABLE {
                if pattern.len() == 2 && two_bytes == pattern {
                    dst[dst_pos] = unicode;
                    src_pos += 2;
                    dst_pos += 1;
                    continue 'outer;
                }
            }
        }

        // Check for single-byte sequences
        let one_byte = &src[src_pos..src_pos + 1];
        for &(pattern, unicode) in TCVN3_DECODE_TABLE {
            if pattern.len() == 1 && one_byte == pattern {
                dst[dst_pos] = unicode;
                src_pos += 1;
                dst_pos += 1;
                continue 'outer;
            }
        }

        // Unknown byte - use replacement
        if last || src_pos + 1 < src.len() {
            dst[dst_pos] = 0xFFFD; // Unicode replacement character
            src_pos += 1;
            dst_pos += 1;
        } else {
            // Need more input
            decoder.pending = Some(first_byte);
            return (DecoderResult::InputEmpty, src_pos, dst_pos);
        }
    }
}

pub struct Tcvn3Encoder;

impl Tcvn3Encoder {
    pub fn new(encoding: &'static Encoding) -> Encoder {
        Encoder::new(
            encoding,
            VariantEncoder::Tcvn3(Tcvn3Encoder),
        )
    }

    pub fn max_buffer_length_from_utf8_if_no_unmappables(&self, byte_length: usize) -> Option<usize> {
        // In worst case, each UTF-8 byte could become 2 TCVN3 bytes
        byte_length.checked_mul(2)
    }

    pub fn max_buffer_length_from_utf16_if_no_unmappables(&self, u16_length: usize) -> Option<usize> {
        // In worst case, each UTF-16 code unit could become 2 TCVN3 bytes
        u16_length.checked_mul(2)
    }

    encoder_functions!(
        {
            // ASCII fast path
            if let Some((read, written)) = ascii_to_ascii_stride(src, dst) {
                return (EncoderResult::InputEmpty, read, written);
            }
            tcvn3_encode_from_utf8_impl(src, dst)
        },
        {
            tcvn3_encode_from_utf16_impl(src, dst)
        },
        {},
        {},
        {},
        eof = {}
    );
}

fn tcvn3_encode_from_utf8_impl(src: &str, dst: &mut [u8]) -> (EncoderResult, usize, usize) {
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;
    
    for ch in src.chars() {
        let unicode = ch as u32;
        
        // ASCII pass-through
        if unicode < 0x80 {
            if dst_pos >= dst.len() {
                return (EncoderResult::OutputFull, src_pos, dst_pos);
            }
            dst[dst_pos] = unicode as u8;
            dst_pos += 1;
            src_pos += ch.len_utf8();
            continue;
        }
        
        // Look up in encode table
        let mut found = false;
        for &(code, bytes) in TCVN3_ENCODE_TABLE {
            if code == unicode as u16 {
                if dst_pos + bytes.len() > dst.len() {
                    return (EncoderResult::OutputFull, src_pos, dst_pos);
                }
                dst[dst_pos..dst_pos + bytes.len()].copy_from_slice(bytes);
                dst_pos += bytes.len();
                found = true;
                break;
            }
        }
        
        if !found {
            // Unmappable character
            return (EncoderResult::Unmappable(ch), src_pos, dst_pos);
        }
        
        src_pos += ch.len_utf8();
    }
    
    (EncoderResult::InputEmpty, src_pos, dst_pos)
}

fn tcvn3_encode_from_utf16_impl(src: &[u16], dst: &mut [u8]) -> (EncoderResult, usize, usize) {
    let mut src_pos = 0usize;
    let mut dst_pos = 0usize;
    
    while src_pos < src.len() {
        let code_unit = src[src_pos];
        
        // ASCII pass-through
        if code_unit < 0x80 {
            if dst_pos >= dst.len() {
                return (EncoderResult::OutputFull, src_pos, dst_pos);
            }
            dst[dst_pos] = code_unit as u8;
            dst_pos += 1;
            src_pos += 1;
            continue;
        }
        
        // Look up in encode table
        let mut found = false;
        for &(unicode, bytes) in TCVN3_ENCODE_TABLE {
            if unicode == code_unit {
                if dst_pos + bytes.len() > dst.len() {
                    return (EncoderResult::OutputFull, src_pos, dst_pos);
                }
                dst[dst_pos..dst_pos + bytes.len()].copy_from_slice(bytes);
                dst_pos += bytes.len();
                found = true;
                break;
            }
        }
        
        if !found {
            // Unmappable character
            let ch = unsafe { char::from_u32_unchecked(code_unit as u32) };
            return (EncoderResult::Unmappable(ch), src_pos, dst_pos);
        }
        
        src_pos += 1;
    }
    
    (EncoderResult::InputEmpty, src_pos, dst_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_tcvn3_decode_single_byte() {
        let mut decoder = Tcvn3Decoder::new();
        if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
            let input = b"\xA2"; // Â
            let mut output = [0u8; 8];
            let (result, read, written) = dec.decode_to_utf8_raw(input, &mut output, true);
            assert_eq!(result, DecoderResult::InputEmpty);
            assert_eq!(read, 1);
            // UTF-8 encoding of Â
            let expected = "Â".as_bytes();
            assert_eq!(&output[..written], expected);
        }
    }

    #[test]
    fn test_tcvn3_decode_two_byte() {
        let mut decoder = Tcvn3Decoder::new();
        if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
            let input = b"A\xB5"; // À
            let mut output = [0u8; 8];
            let (result, read, written) = dec.decode_to_utf8_raw(input, &mut output, true);
            assert_eq!(result, DecoderResult::InputEmpty);
            assert_eq!(read, 2);
            // UTF-8 encoding of À
            let expected = "À".as_bytes();
            assert_eq!(&output[..written], expected);
        }
    }

    #[test]
    fn test_tcvn3_encode_basic() {
        let mut encoder = Tcvn3Encoder;
        let input = "Â";
        let mut output = [0u8; 8];
        let (result, read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
        assert_eq!(result, EncoderResult::InputEmpty);
        assert_eq!(read, input.len());
        assert_eq!(written, 1);
        assert_eq!(output[0], 0xA2);
    }
}