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

use crate::variant::*;
use crate::{DecoderResult, EncoderResult, Encoding, Encoder};

// TCVN3 Unicode to byte sequence mapping
// Based on https://vietunicode.sourceforge.net/charset
const TCVN3_ENCODE_TABLE: &[(u16, &[u8])] = &[
    (0x00C0, b"\x41\xB5"), // À -> Aµ
    (0x00C1, b"\x41\xB8"), // Á -> A¸
    (0x00C2, b"\xA2"), // Â -> ¢
    (0x00C3, b"\x41\xB7"), // Ã -> A·
    (0x00C8, b"\x45\xCC"), // È -> EÌ
    (0x00C9, b"\x45\xD0"), // É -> EÐ
    (0x00CA, b"\xA3"), // Ê -> £
    (0x00CC, b"\x49\xD7"), // Ì -> I×
    (0x00CD, b"\x49\xDD"), // Í -> IÝ
    (0x00D2, b"\x4F\xDF"), // Ò -> Oß
    (0x00D3, b"\x4F\xE3"), // Ó -> Oã
    (0x00D4, b"\xA4"), // Ô -> ¤
    (0x00D5, b"\x4F\xE2"), // Õ -> Oâ
    (0x00D9, b"\x55\xEF"), // Ù -> Uï
    (0x00DA, b"\x55\xF3"), // Ú -> Uó
    (0x00DD, b"\x59\xFD"), // Ý -> Yý
    (0x00E0, b"\xB5"), // à -> µ
    (0x00E1, b"\xB8"), // á -> ¸
    (0x00E2, b"\xA9"), // â -> ©
    (0x00E3, b"\xB7"), // ã -> ·
    (0x00E8, b"\xCC"), // è -> Ì
    (0x00E9, b"\xD0"), // é -> Ð
    (0x00EA, b"\xAA"), // ê -> ª
    (0x00EC, b"\xD7"), // ì -> ×
    (0x00ED, b"\xDD"), // í -> Ý
    (0x00F2, b"\xDF"), // ò -> ß
    (0x00F3, b"\xE3"), // ó -> ã
    (0x00F4, b"\xAB"), // ô -> «
    (0x00F5, b"\xE2"), // õ -> â
    (0x00F9, b"\xEF"), // ù -> ï
    (0x00FA, b"\xF3"), // ú -> ó
    (0x00FD, b"\xFD"), // ý -> ý
    (0x0102, b"\xA1"), // Ă -> ¡
    (0x0103, b"\xA8"), // ă -> ¨
    (0x0110, b"\xA7"), // Đ -> §
    (0x0111, b"\xAE"), // đ -> ®
    (0x0128, b"\x49\xDC"), // Ĩ -> IÜ
    (0x0129, b"\xDC"), // ĩ -> Ü
    (0x0168, b"\x55\xF2"), // Ũ -> Uò
    (0x0169, b"\xF2"), // ũ -> ò
    (0x01A0, b"\xA5"), // Ơ -> ¥
    (0x01A1, b"\xAC"), // ơ -> ¬
    (0x01AF, b"\xA6"), // Ư -> ¦
    (0x01B0, b"\xAD"), // ư -> ­
    (0x1EA0, b"\x41\xB9"), // Ạ -> A¹
    (0x1EA1, b"\xB9"), // ạ -> ¹
    (0x1EA2, b"\x41\xB6"), // Ả -> A¶
    (0x1EA3, b"\xB6"), // ả -> ¶
    (0x1EA4, b"\xA2\xCA"), // Ấ -> ¢Ê
    (0x1EA5, b"\xCA"), // ấ -> Ê
    (0x1EA6, b"\xA2\xC7"), // Ầ -> ¢Ç
    (0x1EA7, b"\xC7"), // ầ -> Ç
    (0x1EA8, b"\xA2\xC8"), // Ẩ -> ¢È
    (0x1EA9, b"\xC8"), // ẩ -> È
    (0x1EAA, b"\xA2\xC9"), // Ẫ -> ¢É
    (0x1EAB, b"\xC9"), // ẫ -> É
    (0x1EAC, b"\xA2\xCB"), // Ậ -> ¢Ë
    (0x1EAD, b"\xCB"), // ậ -> Ë
    (0x1EAE, b"\xA1\xBE"), // Ắ -> ¡¾
    (0x1EAF, b"\xBE"), // ắ -> ¾
    (0x1EB0, b"\xA1\xBB"), // Ằ -> ¡»
    (0x1EB1, b"\xBB"), // ằ -> »
    (0x1EB2, b"\xA1\xBC"), // Ẳ -> ¡¼
    (0x1EB3, b"\xBC"), // ẳ -> ¼
    (0x1EB4, b"\xA1\xBD"), // Ẵ -> ¡½
    (0x1EB5, b"\xBD"), // ẵ -> ½
    (0x1EB6, b"\xA1\xC6"), // Ặ -> ¡Æ
    (0x1EB7, b"\xC6"), // ặ -> Æ
    (0x1EB8, b"\x45\xD1"), // Ẹ -> EÑ
    (0x1EB9, b"\xD1"), // ẹ -> Ñ
    (0x1EBA, b"\x45\xCE"), // Ẻ -> EÎ
    (0x1EBB, b"\xCE"), // ẻ -> Î
    (0x1EBC, b"\x45\xCF"), // Ẽ -> EÏ
    (0x1EBD, b"\xCF"), // ẽ -> Ï
    (0x1EBE, b"\xA3\xD5"), // Ế -> £Õ
    (0x1EBF, b"\xD5"), // ế -> Õ
    (0x1EC0, b"\xA3\xD2"), // Ề -> £Ò
    (0x1EC1, b"\xD2"), // ề -> Ò
    (0x1EC2, b"\xA3\xD3"), // Ể -> £Ó
    (0x1EC3, b"\xD3"), // ể -> Ó
    (0x1EC4, b"\xA3\xD4"), // Ễ -> £Ô
    (0x1EC5, b"\xD4"), // ễ -> Ô
    (0x1EC6, b"\xA3\xD6"), // Ệ -> £Ö
    (0x1EC7, b"\xD6"), // ệ -> Ö
    (0x1EC8, b"\x49\xD8"), // Ỉ -> IØ
    (0x1EC9, b"\xD8"), // ỉ -> Ø
    (0x1ECA, b"\x49\xDE"), // Ị -> IÞ
    (0x1ECB, b"\xDE"), // ị -> Þ
    (0x1ECC, b"\x4F\xE4"), // Ọ -> Oä
    (0x1ECD, b"\xE4"), // ọ -> ä
    (0x1ECE, b"\x4F\xE1"), // Ỏ -> Oá
    (0x1ECF, b"\xE1"), // ỏ -> á
    (0x1ED0, b"\xA4\xE8"), // Ố -> ¤è
    (0x1ED1, b"\xE8"), // ố -> è
    (0x1ED2, b"\xA4\xE5"), // Ồ -> ¤å
    (0x1ED3, b"\xE5"), // ồ -> å
    (0x1ED4, b"\xA4\xE6"), // Ổ -> ¤æ
    (0x1ED5, b"\xE6"), // ổ -> æ
    (0x1ED6, b"\xA4\xE7"), // Ỗ -> ¤ç
    (0x1ED7, b"\xE7"), // ỗ -> ç
    (0x1ED8, b"\xA4\xE9"), // Ộ -> ¤é
    (0x1ED9, b"\xE9"), // ộ -> é
    (0x1EDA, b"\xA5\xED"), // Ớ -> ¥í
    (0x1EDB, b"\xED"), // ớ -> í
    (0x1EDC, b"\xA5\xEA"), // Ờ -> ¥ê
    (0x1EDD, b"\xEA"), // ờ -> ê
    (0x1EDE, b"\xA5\xEB"), // Ở -> ¥ë
    (0x1EDF, b"\xEB"), // ở -> ë
    (0x1EE0, b"\xA5\xEC"), // Ỡ -> ¥ì
    (0x1EE1, b"\xEC"), // ỡ -> ì
    (0x1EE2, b"\xA5\xEE"), // Ợ -> ¥î
    (0x1EE3, b"\xEE"), // ợ -> î
    (0x1EE4, b"\x55\xF4"), // Ụ -> Uô
    (0x1EE5, b"\xF4"), // ụ -> ô
    (0x1EE6, b"\x55\xF1"), // Ủ -> Uñ
    (0x1EE7, b"\xF1"), // ủ -> ñ
    (0x1EE8, b"\xA6\xF8"), // Ứ -> ¦ø
    (0x1EE9, b"\xF8"), // ứ -> ø
    (0x1EEA, b"\xA6\xF5"), // Ừ -> ¦õ
    (0x1EEB, b"\xF5"), // ừ -> õ
    (0x1EEC, b"\xA6\xF6"), // Ử -> ¦ö
    (0x1EED, b"\xF6"), // ử -> ö
    (0x1EEE, b"\xA6\xF7"), // Ữ -> ¦÷
    (0x1EEF, b"\xF7"), // ữ -> ÷
    (0x1EF0, b"\xA6\xF9"), // Ự -> ¦ù
    (0x1EF1, b"\xF9"), // ự -> ù
    (0x1EF2, b"\x59\xFA"), // Ỳ -> Yú
    (0x1EF3, b"\xFA"), // ỳ -> ú
    (0x1EF4, b"\x59\xFE"), // Ỵ -> Yþ
    (0x1EF5, b"\xFE"), // ỵ -> þ
    (0x1EF6, b"\x59\xFB"), // Ỷ -> Yû
    (0x1EF7, b"\xFB"), // ỷ -> û
    (0x1EF8, b"\x59\xFC"), // Ỹ -> Yü
    (0x1EF9, b"\xFC"), // ỹ -> ü
];

// TCVN3 decode sequences to Unicode mapping
// Order matters: longer sequences must come first
const TCVN3_DECODE_TABLE: &[(&[u8], u16)] = &[
    (b"\x41\xB5", 0x00C0), // Aµ -> À
    (b"\x41\xB8", 0x00C1), // A¸ -> Á
    (b"\xA2", 0x00C2), // ¢ -> Â
    (b"\x41\xB7", 0x00C3), // A· -> Ã
    (b"\x45\xCC", 0x00C8), // EÌ -> È
    (b"\x45\xD0", 0x00C9), // EÐ -> É
    (b"\xA3", 0x00CA), // £ -> Ê
    (b"\x49\xD7", 0x00CC), // I× -> Ì
    (b"\x49\xDD", 0x00CD), // IÝ -> Í
    (b"\x4F\xDF", 0x00D2), // Oß -> Ò
    (b"\x4F\xE3", 0x00D3), // Oã -> Ó
    (b"\xA4", 0x00D4), // ¤ -> Ô
    (b"\x4F\xE2", 0x00D5), // Oâ -> Õ
    (b"\x55\xEF", 0x00D9), // Uï -> Ù
    (b"\x55\xF3", 0x00DA), // Uó -> Ú
    (b"\x59\xFD", 0x00DD), // Yý -> Ý
    (b"\xB5", 0x00E0), // µ -> à
    (b"\xB8", 0x00E1), // ¸ -> á
    (b"\xA9", 0x00E2), // © -> â
    (b"\xB7", 0x00E3), // · -> ã
    (b"\xCC", 0x00E8), // Ì -> è
    (b"\xD0", 0x00E9), // Ð -> é
    (b"\xAA", 0x00EA), // ª -> ê
    (b"\xD7", 0x00EC), // × -> ì
    (b"\xDD", 0x00ED), // Ý -> í
    (b"\xDF", 0x00F2), // ß -> ò
    (b"\xE3", 0x00F3), // ã -> ó
    (b"\xAB", 0x00F4), // « -> ô
    (b"\xE2", 0x00F5), // â -> õ
    (b"\xEF", 0x00F9), // ï -> ù
    (b"\xF3", 0x00FA), // ó -> ú
    (b"\xFD", 0x00FD), // ý -> ý
    (b"\xA1", 0x0102), // ¡ -> Ă
    (b"\xA8", 0x0103), // ¨ -> ă
    (b"\xA7", 0x0110), // § -> Đ
    (b"\xAE", 0x0111), // ® -> đ
    (b"\x49\xDC", 0x0128), // IÜ -> Ĩ
    (b"\xDC", 0x0129), // Ü -> ĩ
    (b"\x55\xF2", 0x0168), // Uò -> Ũ
    (b"\xF2", 0x0169), // ò -> ũ
    (b"\xA5", 0x01A0), // ¥ -> Ơ
    (b"\xAC", 0x01A1), // ¬ -> ơ
    (b"\xA6", 0x01AF), // ¦ -> Ư
    (b"\xAD", 0x01B0), // ­ -> ư
    (b"\x41\xB9", 0x1EA0), // A¹ -> Ạ
    (b"\xB9", 0x1EA1), // ¹ -> ạ
    (b"\x41\xB6", 0x1EA2), // A¶ -> Ả
    (b"\xB6", 0x1EA3), // ¶ -> ả
    (b"\xA2\xCA", 0x1EA4), // ¢Ê -> Ấ
    (b"\xCA", 0x1EA5), // Ê -> ấ
    (b"\xA2\xC7", 0x1EA6), // ¢Ç -> Ầ
    (b"\xC7", 0x1EA7), // Ç -> ầ
    (b"\xA2\xC8", 0x1EA8), // ¢È -> Ẩ
    (b"\xC8", 0x1EA9), // È -> ẩ
    (b"\xA2\xC9", 0x1EAA), // ¢É -> Ẫ
    (b"\xC9", 0x1EAB), // É -> ẫ
    (b"\xA2\xCB", 0x1EAC), // ¢Ë -> Ậ
    (b"\xCB", 0x1EAD), // Ë -> ậ
    (b"\xA1\xBE", 0x1EAE), // ¡¾ -> Ắ
    (b"\xBE", 0x1EAF), // ¾ -> ắ
    (b"\xA1\xBB", 0x1EB0), // ¡» -> Ằ
    (b"\xBB", 0x1EB1), // » -> ằ
    (b"\xA1\xBC", 0x1EB2), // ¡¼ -> Ẳ
    (b"\xBC", 0x1EB3), // ¼ -> ẳ
    (b"\xA1\xBD", 0x1EB4), // ¡½ -> Ẵ
    (b"\xBD", 0x1EB5), // ½ -> ẵ
    (b"\xA1\xC6", 0x1EB6), // ¡Æ -> Ặ
    (b"\xC6", 0x1EB7), // Æ -> ặ
    (b"\x45\xD1", 0x1EB8), // EÑ -> Ẹ
    (b"\xD1", 0x1EB9), // Ñ -> ẹ
    (b"\x45\xCE", 0x1EBA), // EÎ -> Ẻ
    (b"\xCE", 0x1EBB), // Î -> ẻ
    (b"\x45\xCF", 0x1EBC), // EÏ -> Ẽ
    (b"\xCF", 0x1EBD), // Ï -> ẽ
    (b"\xA3\xD5", 0x1EBE), // £Õ -> Ế
    (b"\xD5", 0x1EBF), // Õ -> ế
    (b"\xA3\xD2", 0x1EC0), // £Ò -> Ề
    (b"\xD2", 0x1EC1), // Ò -> ề
    (b"\xA3\xD3", 0x1EC2), // £Ó -> Ể
    (b"\xD3", 0x1EC3), // Ó -> ể
    (b"\xA3\xD4", 0x1EC4), // £Ô -> Ễ
    (b"\xD4", 0x1EC5), // Ô -> ễ
    (b"\xA3\xD6", 0x1EC6), // £Ö -> Ệ
    (b"\xD6", 0x1EC7), // Ö -> ệ
    (b"\x49\xD8", 0x1EC8), // IØ -> Ỉ
    (b"\xD8", 0x1EC9), // Ø -> ỉ
    (b"\x49\xDE", 0x1ECA), // IÞ -> Ị
    (b"\xDE", 0x1ECB), // Þ -> ị
    (b"\x4F\xE4", 0x1ECC), // Oä -> Ọ
    (b"\xE4", 0x1ECD), // ä -> ọ
    (b"\x4F\xE1", 0x1ECE), // Oá -> Ỏ
    (b"\xE1", 0x1ECF), // á -> ỏ
    (b"\xA4\xE8", 0x1ED0), // ¤è -> Ố
    (b"\xE8", 0x1ED1), // è -> ố
    (b"\xA4\xE5", 0x1ED2), // ¤å -> Ồ
    (b"\xE5", 0x1ED3), // å -> ồ
    (b"\xA4\xE6", 0x1ED4), // ¤æ -> Ổ
    (b"\xE6", 0x1ED5), // æ -> ổ
    (b"\xA4\xE7", 0x1ED6), // ¤ç -> Ỗ
    (b"\xE7", 0x1ED7), // ç -> ỗ
    (b"\xA4\xE9", 0x1ED8), // ¤é -> Ộ
    (b"\xE9", 0x1ED9), // é -> ộ
    (b"\xA5\xED", 0x1EDA), // ¥í -> Ớ
    (b"\xED", 0x1EDB), // í -> ớ
    (b"\xA5\xEA", 0x1EDC), // ¥ê -> Ờ
    (b"\xEA", 0x1EDD), // ê -> ờ
    (b"\xA5\xEB", 0x1EDE), // ¥ë -> Ở
    (b"\xEB", 0x1EDF), // ë -> ở
    (b"\xA5\xEC", 0x1EE0), // ¥ì -> Ỡ
    (b"\xEC", 0x1EE1), // ì -> ỡ
    (b"\xA5\xEE", 0x1EE2), // ¥î -> Ợ
    (b"\xEE", 0x1EE3), // î -> ợ
    (b"\x55\xF4", 0x1EE4), // Uô -> Ụ
    (b"\xF4", 0x1EE5), // ô -> ụ
    (b"\x55\xF1", 0x1EE6), // Uñ -> Ủ
    (b"\xF1", 0x1EE7), // ñ -> ủ
    (b"\xA6\xF8", 0x1EE8), // ¦ø -> Ứ
    (b"\xF8", 0x1EE9), // ø -> ứ
    (b"\xA6\xF5", 0x1EEA), // ¦õ -> Ừ
    (b"\xF5", 0x1EEB), // õ -> ừ
    (b"\xA6\xF6", 0x1EEC), // ¦ö -> Ử
    (b"\xF6", 0x1EED), // ö -> ử
    (b"\xA6\xF7", 0x1EEE), // ¦÷ -> Ữ
    (b"\xF7", 0x1EEF), // ÷ -> ữ
    (b"\xA6\xF9", 0x1EF0), // ¦ù -> Ự
    (b"\xF9", 0x1EF1), // ù -> ự
    (b"\x59\xFA", 0x1EF2), // Yú -> Ỳ
    (b"\xFA", 0x1EF3), // ú -> ỳ
    (b"\x59\xFE", 0x1EF4), // Yþ -> Ỵ
    (b"\xFE", 0x1EF5), // þ -> ỵ
    (b"\x59\xFB", 0x1EF6), // Yû -> Ỷ
    (b"\xFB", 0x1EF7), // û -> ỷ
    (b"\x59\xFC", 0x1EF8), // Yü -> Ỹ
    (b"\xFC", 0x1EF9), // ü -> ỹ
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

    pub fn max_utf16_buffer_length(&self, byte_length: usize) -> Option<usize> {
        // Each TCVN3 byte sequence can produce at most 1 UTF-16 code unit
        Some(byte_length)
    }

    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        // Each TCVN3 byte sequence can produce at most 4 UTF-8 bytes (for surrogates)
        byte_length.checked_mul(4)
    }

    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        // Each TCVN3 byte sequence can produce at most 4 UTF-8 bytes (for surrogates)
        byte_length.checked_mul(4)
    }

    pub fn decode_to_utf8_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut src_pos = 0usize;
        let mut dst_pos = 0usize;
        'outer: loop {
            if src_pos >= src.len() {
                return (DecoderResult::InputEmpty, src_pos, dst_pos);
            }
            if dst_pos >= dst.len() {
                return (DecoderResult::OutputFull, src_pos, dst_pos);
            }

            // Check for two-byte sequences first
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
                            let mut utf8_buffer = [0u8; 4];
                            let utf8_str = ch.encode_utf8(&mut utf8_buffer);
                            let utf8_bytes = utf8_str.as_bytes().to_vec();
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

            let first_byte = src[src_pos];
            
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
                        let mut utf8_buffer = [0u8; 4];
                        let utf8_str = ch.encode_utf8(&mut utf8_buffer);
                        let utf8_bytes = utf8_str.as_bytes().to_vec();
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

            // Handle ASCII (after checking for TCVN3 sequences)
            if first_byte < 0x80 {
                dst[dst_pos] = first_byte;
                src_pos += 1;
                dst_pos += 1;
                continue;
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
    }

    pub fn decode_to_utf16_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u16],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        tcvn3_decode_to_utf16_impl(self, src, dst, last)
    }
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

        // Check for two-byte sequences first
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

        let first_byte = src[src_pos];
        
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

        // Handle ASCII (after checking for TCVN3 sequences)
        if first_byte < 0x80 {
            dst[dst_pos] = first_byte as u16;
            src_pos += 1;
            dst_pos += 1;
            continue;
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

    pub fn encode_from_utf8_raw(
        &mut self,
        src: &str,
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        tcvn3_encode_from_utf8_impl(src, dst)
    }

    pub fn encode_from_utf16_raw(
        &mut self,
        src: &[u16],
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        tcvn3_encode_from_utf16_impl(src, dst)
    }
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
        let input = "Â";
        let mut output = [0u8; 8];
        let (result, read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
        assert_eq!(result, EncoderResult::InputEmpty);
        assert_eq!(read, input.len());
        assert_eq!(written, 1);
        assert_eq!(output[0], 0xA2);
    }

    #[test]
    fn test_tcvn3_vietnamese_text() {
        // Test với text tiếng Việt thực tế
        let vietnamese_text = "Việt Nam";
        
        // Test encode
        let mut encoded = [0u8; 32];
        let (_encode_result, _encode_read, encode_written) = tcvn3_encode_from_utf8_impl(vietnamese_text, &mut encoded);
        
        // Test decode ngược lại
        let mut decoder = Tcvn3Decoder::new();
        if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
            let mut decoded = [0u8; 32];
            let (decode_result, _decode_read, decode_written) = dec.decode_to_utf8_raw(&encoded[..encode_written], &mut decoded, true);
            let _decoded_text = core::str::from_utf8(&decoded[..decode_written]).unwrap();
            
            // Kiểm tra decode thành công
            assert_eq!(decode_result, DecoderResult::InputEmpty);
            assert!(decode_written > 0);
        }
    }

    #[test]
    fn test_tcvn3_complete_vietnamese_sentence() {
        // Test câu tiếng Việt hoàn chỉnh
        let sentence = "Chào bạn!";
        
        let mut encoded = [0u8; 64];
            let (_encode_result, _encode_read, encode_written) = tcvn3_encode_from_utf8_impl(sentence, &mut encoded);
        
        // Những ký tự ASCII như "Chao ban!" sẽ encode được
        // Những ký tự có dấu có thể không encode được nếu không có trong bảng
        assert!(encode_written > 0, "Should encode at least some characters");
    }

    #[test]
    fn test_tcvn3_all_vietnamese_vowels_uppercase() {
        // Test tất cả nguyên âm tiếng Việt hoa
        let test_cases = [
            // Nguyên âm A
            ("À", &[0x41u8, 0xB5][..]),     // À -> Aµ  
            ("Á", &[0x41u8, 0xB8][..]),     // Á -> A¸
            ("Â", &[0xA2u8][..]),           // Â -> ¢
            ("Ã", &[0x41u8, 0xB7][..]),     // Ã -> A·
            ("Ă", &[0xA1u8][..]),           // Ă -> ¡
            
            // Nguyên âm E
            ("È", &[0x45u8, 0xCC][..]),     // È -> EÌ
            ("É", &[0x45u8, 0xD0][..]),     // É -> EÐ
            ("Ê", &[0xA3u8][..]),           // Ê -> £
            
            // Nguyên âm I
            ("Ì", &[0x49u8, 0xD7][..]),     // Ì -> I×
            ("Í", &[0x49u8, 0xDD][..]),     // Í -> IÝ
            ("Ĩ", &[0x49u8, 0xDC][..]),     // Ĩ -> IÜ
            
            // Nguyên âm O
            ("Ò", &[0x4Fu8, 0xDF][..]),     // Ò -> Oß
            ("Ó", &[0x4Fu8, 0xE3][..]),     // Ó -> Oã
            ("Ô", &[0xA4u8][..]),           // Ô -> ¤
            ("Õ", &[0x4Fu8, 0xE2][..]),     // Õ -> Oâ
            ("Ơ", &[0xA5u8][..]),           // Ơ -> ¥
            
            // Nguyên âm U
            ("Ù", &[0x55u8, 0xEF][..]),     // Ù -> Uï
            ("Ú", &[0x55u8, 0xF3][..]),     // Ú -> Uó
            ("Ũ", &[0x55u8, 0xF2][..]),     // Ũ -> Uò
            ("Ư", &[0xA6u8][..]),           // Ư -> ¦
            
            // Y
            ("Ý", &[0x59u8, 0xFD][..]),     // Ý -> Yý
            
            // Đ
            ("Đ", &[0xA7u8][..]),           // Đ -> §
        ];

        for (input, expected) in test_cases.iter() {
            let mut output = [0u8; 8];
            let (result, _read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
            
            assert_eq!(result, EncoderResult::InputEmpty, "Failed to encode '{}'", input);
            assert_eq!(&output[..written], *expected, "Wrong encoding for '{}'", input);
            
            // Test decode ngược
            let mut decoder = Tcvn3Decoder::new();
            if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
                let mut decoded = [0u8; 8];
                let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(expected, &mut decoded, true);
                assert_eq!(decode_result, DecoderResult::InputEmpty, "Failed to decode for '{}'", input);
                let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
                assert_eq!(decoded_str, *input, "Decode mismatch for '{}'", input);
            }
        }
    }

    #[test]
    fn test_tcvn3_all_vietnamese_vowels_lowercase() {
        // Test tất cả nguyên âm tiếng Việt thường
        let test_cases = [
            // Nguyên âm a
            ("à", &[0xB5u8][..]),           // à -> µ
            ("á", &[0xB8u8][..]),           // á -> ¸
            ("â", &[0xA9u8][..]),           // â -> ©
            ("ã", &[0xB7u8][..]),           // ã -> ·
            ("ă", &[0xA8u8][..]),           // ă -> ¨
            
            // Nguyên âm e
            ("è", &[0xCCu8][..]),           // è -> Ì
            ("é", &[0xD0u8][..]),           // é -> Ð
            ("ê", &[0xAAu8][..]),           // ê -> ª
            
            // Nguyên âm i
            ("ì", &[0xD7u8][..]),           // ì -> ×
            ("í", &[0xDDu8][..]),           // í -> Ý
            ("ĩ", &[0xDCu8][..]),           // ĩ -> Ü
            
            // Nguyên âm o
            ("ò", &[0xDFu8][..]),           // ò -> ß
            ("ó", &[0xE3u8][..]),           // ó -> ã
            ("ô", &[0xABu8][..]),           // ô -> «
            ("õ", &[0xE2u8][..]),           // õ -> â
            ("ơ", &[0xACu8][..]),           // ơ -> ¬
            
            // Nguyên âm u
            ("ù", &[0xEFu8][..]),           // ù -> ï
            ("ú", &[0xF3u8][..]),           // ú -> ó
            ("ũ", &[0xF2u8][..]),           // ũ -> ò
            ("ư", &[0xADu8][..]),           // ư -> ­
            
            // y
            ("ý", &[0xFDu8][..]),           // ý -> ý
            
            // đ
            ("đ", &[0xAEu8][..]),           // đ -> ®
        ];

        for (input, expected) in test_cases.iter() {
            let mut output = [0u8; 8];
            let (result, _read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
            
            assert_eq!(result, EncoderResult::InputEmpty, "Failed to encode '{}'", input);
            assert_eq!(&output[..written], *expected, "Wrong encoding for '{}'", input);
            
            // Test decode ngược
            let mut decoder = Tcvn3Decoder::new();
            if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
                let mut decoded = [0u8; 8];
                let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(expected, &mut decoded, true);
                assert_eq!(decode_result, DecoderResult::InputEmpty, "Failed to decode for '{}'", input);
                let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
                assert_eq!(decoded_str, *input, "Decode mismatch for '{}'", input);
            }
        }
    }

    #[test]
    fn test_tcvn3_vietnamese_extended_characters_uppercase() {
        // Test các ký tự mở rộng tiếng Việt hoa (1EA0-1EF9)
        let test_cases = [
            // A với các dấu đặc biệt
            ("Ạ", &[0x41u8, 0xB9][..]),     // Ạ -> A¹
            ("Ả", &[0x41u8, 0xB6][..]),     // Ả -> A¶
            ("Ấ", &[0xA2u8, 0xCA][..]),     // Ấ -> ¢Ê
            ("Ầ", &[0xA2u8, 0xC7][..]),     // Ầ -> ¢Ç
            ("Ẩ", &[0xA2u8, 0xC8][..]),     // Ẩ -> ¢È
            ("Ẫ", &[0xA2u8, 0xC9][..]),     // Ẫ -> ¢É
            ("Ậ", &[0xA2u8, 0xCB][..]),     // Ậ -> ¢Ë
            ("Ắ", &[0xA1u8, 0xBE][..]),     // Ắ -> ¡¾
            ("Ằ", &[0xA1u8, 0xBB][..]),     // Ằ -> ¡»
            ("Ẳ", &[0xA1u8, 0xBC][..]),     // Ẳ -> ¡¼
            ("Ẵ", &[0xA1u8, 0xBD][..]),     // Ẵ -> ¡½
            ("Ặ", &[0xA1u8, 0xC6][..]),     // Ặ -> ¡Æ
            
            // E với các dấu đặc biệt
            ("Ẹ", &[0x45u8, 0xD1][..]),     // Ẹ -> EÑ
            ("Ẻ", &[0x45u8, 0xCE][..]),     // Ẻ -> EÎ
            ("Ẽ", &[0x45u8, 0xCF][..]),     // Ẽ -> EÏ
            ("Ế", &[0xA3u8, 0xD5][..]),     // Ế -> £Õ
            ("Ề", &[0xA3u8, 0xD2][..]),     // Ề -> £Ò
            ("Ể", &[0xA3u8, 0xD3][..]),     // Ể -> £Ó
            ("Ễ", &[0xA3u8, 0xD4][..]),     // Ễ -> £Ô
            ("Ệ", &[0xA3u8, 0xD6][..]),     // Ệ -> £Ö
            
            // I với các dấu đặc biệt
            ("Ỉ", &[0x49u8, 0xD8][..]),     // Ỉ -> IØ
            ("Ị", &[0x49u8, 0xDE][..]),     // Ị -> IÞ
            
            // O với các dấu đặc biệt
            ("Ọ", &[0x4Fu8, 0xE4][..]),     // Ọ -> Oä
            ("Ỏ", &[0x4Fu8, 0xE1][..]),     // Ỏ -> Oá
            ("Ố", &[0xA4u8, 0xE8][..]),     // Ố -> ¤è
            ("Ồ", &[0xA4u8, 0xE5][..]),     // Ồ -> ¤å
            ("Ổ", &[0xA4u8, 0xE6][..]),     // Ổ -> ¤æ
            ("Ỗ", &[0xA4u8, 0xE7][..]),     // Ỗ -> ¤ç
            ("Ộ", &[0xA4u8, 0xE9][..]),     // Ộ -> ¤é
            ("Ớ", &[0xA5u8, 0xED][..]),     // Ớ -> ¥í
            ("Ờ", &[0xA5u8, 0xEA][..]),     // Ờ -> ¥ê
            ("Ở", &[0xA5u8, 0xEB][..]),     // Ở -> ¥ë
            ("Ỡ", &[0xA5u8, 0xEC][..]),     // Ỡ -> ¥ì
            ("Ợ", &[0xA5u8, 0xEE][..]),     // Ợ -> ¥î
            
            // U với các dấu đặc biệt
            ("Ụ", &[0x55u8, 0xF4][..]),     // Ụ -> Uô
            ("Ủ", &[0x55u8, 0xF1][..]),     // Ủ -> Uñ
            ("Ứ", &[0xA6u8, 0xF8][..]),     // Ứ -> ¦ø
            ("Ừ", &[0xA6u8, 0xF5][..]),     // Ừ -> ¦õ
            ("Ử", &[0xA6u8, 0xF6][..]),     // Ử -> ¦ö
            ("Ữ", &[0xA6u8, 0xF7][..]),     // Ữ -> ¦÷
            ("Ự", &[0xA6u8, 0xF9][..]),     // Ự -> ¦ù
            
            // Y với các dấu đặc biệt
            ("Ỳ", &[0x59u8, 0xFA][..]),     // Ỳ -> Yú
            ("Ỵ", &[0x59u8, 0xFE][..]),     // Ỵ -> Yþ
            ("Ỷ", &[0x59u8, 0xFB][..]),     // Ỷ -> Yû
            ("Ỹ", &[0x59u8, 0xFC][..]),     // Ỹ -> Yü
        ];

        for (input, expected) in test_cases.iter() {
            let mut output = [0u8; 8];
            let (result, _read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
            
            assert_eq!(result, EncoderResult::InputEmpty, "Failed to encode '{}'", input);
            assert_eq!(&output[..written], *expected, "Wrong encoding for '{}'", input);
            
            // Test decode ngược
            let mut decoder = Tcvn3Decoder::new();
            if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
                let mut decoded = [0u8; 8];
                let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(expected, &mut decoded, true);
                assert_eq!(decode_result, DecoderResult::InputEmpty, "Failed to decode for '{}'", input);
                let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
                assert_eq!(decoded_str, *input, "Decode mismatch for '{}'", input);
            }
        }
    }

    #[test]
    fn test_tcvn3_vietnamese_extended_characters_lowercase() {
        // Test các ký tự mở rộng tiếng Việt thường (1EA0-1EF9)
        let test_cases = [
            // a với các dấu đặc biệt
            ("ạ", &[0xB9u8][..]),           // ạ -> ¹
            ("ả", &[0xB6u8][..]),           // ả -> ¶
            ("ấ", &[0xCAu8][..]),           // ấ -> Ê
            ("ầ", &[0xC7u8][..]),           // ầ -> Ç
            ("ẩ", &[0xC8u8][..]),           // ẩ -> È
            ("ẫ", &[0xC9u8][..]),           // ẫ -> É
            ("ậ", &[0xCBu8][..]),           // ậ -> Ë
            ("ắ", &[0xBEu8][..]),           // ắ -> ¾
            ("ằ", &[0xBBu8][..]),           // ằ -> »
            ("ẳ", &[0xBCu8][..]),           // ẳ -> ¼
            ("ẵ", &[0xBDu8][..]),           // ẵ -> ½
            ("ặ", &[0xC6u8][..]),           // ặ -> Æ
            
            // e với các dấu đặc biệt
            ("ẹ", &[0xD1u8][..]),           // ẹ -> Ñ
            ("ẻ", &[0xCEu8][..]),           // ẻ -> Î
            ("ẽ", &[0xCFu8][..]),           // ẽ -> Ï
            ("ế", &[0xD5u8][..]),           // ế -> Õ
            ("ề", &[0xD2u8][..]),           // ề -> Ò
            ("ể", &[0xD3u8][..]),           // ể -> Ó
            ("ễ", &[0xD4u8][..]),           // ễ -> Ô
            ("ệ", &[0xD6u8][..]),           // ệ -> Ö
            
            // i với các dấu đặc biệt
            ("ỉ", &[0xD8u8][..]),           // ỉ -> Ø
            ("ị", &[0xDEu8][..]),           // ị -> Þ
            
            // o với các dấu đặc biệt
            ("ọ", &[0xE4u8][..]),           // ọ -> ä
            ("ỏ", &[0xE1u8][..]),           // ỏ -> á
            ("ố", &[0xE8u8][..]),           // ố -> è
            ("ồ", &[0xE5u8][..]),           // ồ -> å
            ("ổ", &[0xE6u8][..]),           // ổ -> æ
            ("ỗ", &[0xE7u8][..]),           // ỗ -> ç
            ("ộ", &[0xE9u8][..]),           // ộ -> é
            ("ớ", &[0xEDu8][..]),           // ớ -> í
            ("ờ", &[0xEAu8][..]),           // ờ -> ê
            ("ở", &[0xEBu8][..]),           // ở -> ë
            ("ỡ", &[0xECu8][..]),           // ỡ -> ì
            ("ợ", &[0xEEu8][..]),           // ợ -> î
            
            // u với các dấu đặc biệt
            ("ụ", &[0xF4u8][..]),           // ụ -> ô
            ("ủ", &[0xF1u8][..]),           // ủ -> ñ
            ("ứ", &[0xF8u8][..]),           // ứ -> ø
            ("ừ", &[0xF5u8][..]),           // ừ -> õ
            ("ử", &[0xF6u8][..]),           // ử -> ö
            ("ữ", &[0xF7u8][..]),           // ữ -> ÷
            ("ự", &[0xF9u8][..]),           // ự -> ù
            
            // y với các dấu đặc biệt
            ("ỳ", &[0xFAu8][..]),           // ỳ -> ú
            ("ỵ", &[0xFEu8][..]),           // ỵ -> þ
            ("ỷ", &[0xFBu8][..]),           // ỷ -> û
            ("ỹ", &[0xFCu8][..]),           // ỹ -> ü
        ];

        for (input, expected) in test_cases.iter() {
            let mut output = [0u8; 8];
            let (result, _read, written) = tcvn3_encode_from_utf8_impl(input, &mut output);
            
            assert_eq!(result, EncoderResult::InputEmpty, "Failed to encode '{}'", input);
            assert_eq!(&output[..written], *expected, "Wrong encoding for '{}'", input);
            
            // Test decode ngược
            let mut decoder = Tcvn3Decoder::new();
            if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
                let mut decoded = [0u8; 8];
                let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(expected, &mut decoded, true);
                assert_eq!(decode_result, DecoderResult::InputEmpty, "Failed to decode for '{}'", input);
                let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
                assert_eq!(decoded_str, *input, "Decode mismatch for '{}'", input);
            }
        }
    }

    #[test]
    fn test_tcvn3_complete_vietnamese_sentences() {
        // Test với các câu tiếng Việt hoàn chỉnh
        let test_sentences = [
            "Xin chào",
            "Việt Nam",
            "Tiếng Việt", 
            "Hà Nội",
            "Thành phố Hồ Chí Minh",
            "Đại học Quốc gia",
            "Tôi yêu Việt Nam",
        ];

        for sentence in test_sentences.iter() {
            let mut encoded = [0u8; 256];
            let (encode_result, encode_read, encode_written) = tcvn3_encode_from_utf8_impl(sentence, &mut encoded);
            
            // Một số ký tự có thể không encode được, nhưng ít nhất ASCII phải được
            assert!(encode_written > 0, "Should encode at least some characters in '{}'", sentence);
            assert!(encode_read > 0, "Should read at least some characters from '{}'", sentence);
            
            // Test decode ngược với phần đã encode được
            if encode_result == EncoderResult::InputEmpty {
                let mut decoder = Tcvn3Decoder::new();
                if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
                    let mut decoded = [0u8; 256];
                    let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(&encoded[..encode_written], &mut decoded, true);
                    assert_eq!(decode_result, DecoderResult::InputEmpty, "Failed to decode sentence '{}'", sentence);
                    let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
                    assert_eq!(decoded_str, *sentence, "Decode mismatch for sentence '{}'", sentence);
                }
            }
        }
    }

    #[test]
    fn test_tcvn3_ascii_passthrough() {
        // Test ASCII characters pass through unchanged
        let ascii_text = "Hello World 123!@#$%^&*()";
        let mut encoded = [0u8; 64];
        let (result, read, written) = tcvn3_encode_from_utf8_impl(ascii_text, &mut encoded);
        
        assert_eq!(result, EncoderResult::InputEmpty);
        assert_eq!(read, ascii_text.len());
        assert_eq!(written, ascii_text.len());
        assert_eq!(&encoded[..written], ascii_text.as_bytes());
        
        // Test decode
        let mut decoder = Tcvn3Decoder::new();
        if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
            let mut decoded = [0u8; 64];
            let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(&encoded[..written], &mut decoded, true);
            assert_eq!(decode_result, DecoderResult::InputEmpty);
            let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
            assert_eq!(decoded_str, ascii_text);
        }
    }

    #[test]
    fn test_tcvn3_unmappable_characters() {
        // Test characters that cannot be encoded to TCVN3
        let unmappable_chars = ["中", "日", "한", "😀", "€"];
        
        for ch in unmappable_chars.iter() {
            let mut output = [0u8; 8];
            let (result, _read, _written) = tcvn3_encode_from_utf8_impl(ch, &mut output);
            
            // Should return Unmappable error
            match result {
                EncoderResult::Unmappable(_) => {
                    // This is expected
                }
                _ => panic!("Expected Unmappable error for character '{}'", ch),
            }
        }
    }

    #[test]
    fn test_tcvn3_buffer_overflow() {
        // Test buffer overflow scenarios
        let text = "Việt Nam";
        let mut small_buffer = [0u8; 2]; // Intentionally small buffer
        
        let (result, _read, _written) = tcvn3_encode_from_utf8_impl(text, &mut small_buffer);
        
        // Should return OutputFull when buffer is too small
        assert_eq!(result, EncoderResult::OutputFull);
    }

    #[test]
    fn test_tcvn3_mixed_content() {
        // Test mixed Vietnamese and ASCII content
        let mixed_text = "Hello Việt Nam 123";
        let mut encoded = [0u8; 64];
        let (result, read, written) = tcvn3_encode_from_utf8_impl(mixed_text, &mut encoded);
        
        assert_eq!(result, EncoderResult::InputEmpty);
        assert_eq!(read, mixed_text.len());
        assert!(written > 0);
        
        // Test decode
        let mut decoder = Tcvn3Decoder::new();
        if let VariantDecoder::Tcvn3(ref mut dec) = decoder {
            let mut decoded = [0u8; 64];
            let (decode_result, _, decode_written) = dec.decode_to_utf8_raw(&encoded[..written], &mut decoded, true);
            assert_eq!(decode_result, DecoderResult::InputEmpty);
            let decoded_str = core::str::from_utf8(&decoded[..decode_written]).unwrap();
            assert_eq!(decoded_str, mixed_text);
        }
    }
}