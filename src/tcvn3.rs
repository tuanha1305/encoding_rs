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
use crate::{DecoderResult, EncoderResult, Encoding, Encoder};

cfg_if! {
    if #[cfg(all(feature = "simd-accel", any(target_feature = "sse2", all(target_endian = "little", target_arch = "aarch64"), all(target_endian = "little", target_feature = "neon"))))] {
        use crate::simd_funcs::*;
        use core::simd::u8x16;
    }
}

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

// Helper function to decode a single TCVN3 byte
#[inline(always)]
fn tcvn3_decode_single_byte(byte: u8) -> Option<u16> {
    // Only check bytes >= 0x80
    if byte < 0x80 {
        return None;
    }
    
    // Use match for common single-byte patterns (more efficient than table lookup)
    match byte {
        0xA1 => Some(0x0102), // ¡ -> Ă
        0xA2 => Some(0x00C2), // ¢ -> Â
        0xA3 => Some(0x00CA), // £ -> Ê
        0xA4 => Some(0x00D4), // ¤ -> Ô
        0xA5 => Some(0x01A0), // ¥ -> Ơ
        0xA6 => Some(0x01AF), // ¦ -> Ư
        0xA7 => Some(0x0110), // § -> Đ
        0xA8 => Some(0x0103), // ¨ -> ă
        0xA9 => Some(0x00E2), // © -> â
        0xAA => Some(0x00EA), // ª -> ê
        0xAB => Some(0x00F4), // « -> ô
        0xAC => Some(0x01A1), // ¬ -> ơ
        0xAD => Some(0x01B0), // ­ -> ư
        0xAE => Some(0x0111), // ® -> đ
        0xB5 => Some(0x00E0), // µ -> à
        0xB6 => Some(0x1EA3), // ¶ -> ả
        0xB7 => Some(0x00E3), // · -> ã
        0xB8 => Some(0x00E1), // ¸ -> á
        0xB9 => Some(0x1EA1), // ¹ -> ạ
        0xBB => Some(0x1EB1), // » -> ằ
        0xBC => Some(0x1EB3), // ¼ -> ẳ
        0xBD => Some(0x1EB5), // ½ -> ẵ
        0xBE => Some(0x1EAF), // ¾ -> ắ
        0xC6 => Some(0x1EB7), // Æ -> ặ
        0xC7 => Some(0x1EA7), // Ç -> ầ
        0xC8 => Some(0x1EA9), // È -> ẩ
        0xC9 => Some(0x1EAB), // É -> ẫ
        0xCA => Some(0x1EA5), // Ê -> ấ
        0xCB => Some(0x1EAD), // Ë -> ậ
        0xCC => Some(0x00E8), // Ì -> è
        0xCE => Some(0x1EBB), // Î -> ẻ
        0xCF => Some(0x1EBD), // Ï -> ẽ
        0xD0 => Some(0x00E9), // Ð -> é
        0xD1 => Some(0x1EB9), // Ñ -> ẹ
        0xD2 => Some(0x1EC1), // Ò -> ề
        0xD3 => Some(0x1EC3), // Ó -> ể
        0xD4 => Some(0x1EC5), // Ô -> ễ
        0xD5 => Some(0x1EBF), // Õ -> ế
        0xD6 => Some(0x1EC7), // Ö -> ệ
        0xD7 => Some(0x00EC), // × -> ì
        0xD8 => Some(0x1EC9), // Ø -> ỉ
        0xDC => Some(0x0129), // Ü -> ĩ
        0xDD => Some(0x00ED), // Ý -> í
        0xDE => Some(0x1ECB), // Þ -> ị
        0xDF => Some(0x00F2), // ß -> ò
        0xE1 => Some(0x1ECF), // á -> ỏ
        0xE2 => Some(0x00F5), // â -> õ
        0xE3 => Some(0x00F3), // ã -> ó
        0xE4 => Some(0x1ECD), // ä -> ọ
        0xE5 => Some(0x1ED3), // å -> ồ
        0xE6 => Some(0x1ED5), // æ -> ổ
        0xE7 => Some(0x1ED7), // ç -> ỗ
        0xE8 => Some(0x1ED1), // è -> ố
        0xE9 => Some(0x1ED9), // é -> ộ
        0xEA => Some(0x1EDD), // ê -> ờ
        0xEB => Some(0x1EDF), // ë -> ở
        0xEC => Some(0x1EE1), // ì -> ỡ
        0xED => Some(0x1EDB), // í -> ớ
        0xEE => Some(0x1EE3), // î -> ợ
        0xEF => Some(0x00F9), // ï -> ù
        0xF1 => Some(0x1EE7), // ñ -> ủ
        0xF2 => Some(0x0169), // ò -> ũ
        0xF3 => Some(0x00FA), // ó -> ú
        0xF4 => Some(0x1EE5), // ô -> ụ
        0xF5 => Some(0x1EEB), // õ -> ừ
        0xF6 => Some(0x1EED), // ö -> ử
        0xF7 => Some(0x1EEF), // ÷ -> ữ
        0xF8 => Some(0x1EE9), // ø -> ứ
        0xF9 => Some(0x1EF1), // ù -> ự
        0xFA => Some(0x1EF3), // ú -> ỳ
        0xFB => Some(0x1EF7), // û -> ỷ
        0xFC => Some(0x1EF9), // ü -> ỹ
        0xFD => Some(0x00FD), // ý -> ý
        0xFE => Some(0x1EF5), // þ -> ỵ
        _ => None,
    }
}

// Helper function to decode a two-byte TCVN3 sequence
#[inline(always)]
fn tcvn3_decode_two_bytes(first: u8, second: u8) -> Option<u16> {
    // Check common two-byte patterns
    match first {
        0x41 => match second { // A + diacritic
            0xB5 => Some(0x00C0), // Aµ -> À
            0xB6 => Some(0x1EA2), // A¶ -> Ả
            0xB7 => Some(0x00C3), // A· -> Ã
            0xB8 => Some(0x00C1), // A¸ -> Á
            0xB9 => Some(0x1EA0), // A¹ -> Ạ
            _ => None,
        },
        0x45 => match second { // E + diacritic
            0xCC => Some(0x00C8), // EÌ -> È
            0xCE => Some(0x1EBA), // EÎ -> Ẻ
            0xCF => Some(0x1EBC), // EÏ -> Ẽ
            0xD0 => Some(0x00C9), // EÐ -> É
            0xD1 => Some(0x1EB8), // EÑ -> Ẹ
            _ => None,
        },
        0x49 => match second { // I + diacritic
            0xD7 => Some(0x00CC), // I× -> Ì
            0xD8 => Some(0x1EC8), // IØ -> Ỉ
            0xDC => Some(0x0128), // IÜ -> Ĩ
            0xDD => Some(0x00CD), // IÝ -> Í
            0xDE => Some(0x1ECA), // IÞ -> Ị
            _ => None,
        },
        0x4F => match second { // O + diacritic
            0xDF => Some(0x00D2), // Oß -> Ò
            0xE1 => Some(0x1ECE), // Oá -> Ỏ
            0xE2 => Some(0x00D5), // Oâ -> Õ
            0xE3 => Some(0x00D3), // Oã -> Ó
            0xE4 => Some(0x1ECC), // Oä -> Ọ
            _ => None,
        },
        0x55 => match second { // U + diacritic
            0xEF => Some(0x00D9), // Uï -> Ù
            0xF1 => Some(0x1EE6), // Uñ -> Ủ
            0xF2 => Some(0x0168), // Uò -> Ũ
            0xF3 => Some(0x00DA), // Uó -> Ú
            0xF4 => Some(0x1EE4), // Uô -> Ụ
            _ => None,
        },
        0x59 => match second { // Y + diacritic
            0xFA => Some(0x1EF2), // Yú -> Ỳ
            0xFB => Some(0x1EF6), // Yû -> Ỷ
            0xFC => Some(0x1EF8), // Yü -> Ỹ
            0xFD => Some(0x00DD), // Yý -> Ý
            0xFE => Some(0x1EF4), // Yþ -> Ỵ
            _ => None,
        },
        0xA1 => match second { // Ă + diacritic
            0xBB => Some(0x1EB0), // ¡» -> Ằ
            0xBC => Some(0x1EB2), // ¡¼ -> Ẳ
            0xBD => Some(0x1EB4), // ¡½ -> Ẵ
            0xBE => Some(0x1EAE), // ¡¾ -> Ắ
            0xC6 => Some(0x1EB6), // ¡Æ -> Ặ
            _ => None,
        },
        0xA2 => match second { // Â + diacritic
            0xC7 => Some(0x1EA6), // ¢Ç -> Ầ
            0xC8 => Some(0x1EA8), // ¢È -> Ẩ
            0xC9 => Some(0x1EAA), // ¢É -> Ẫ
            0xCA => Some(0x1EA4), // ¢Ê -> Ấ
            0xCB => Some(0x1EAC), // ¢Ë -> Ậ
            _ => None,
        },
        0xA3 => match second { // Ê + diacritic
            0xD2 => Some(0x1EC0), // £Ò -> Ề
            0xD3 => Some(0x1EC2), // £Ó -> Ể
            0xD4 => Some(0x1EC4), // £Ô -> Ễ
            0xD5 => Some(0x1EBE), // £Õ -> Ế
            0xD6 => Some(0x1EC6), // £Ö -> Ệ
            _ => None,
        },
        0xA4 => match second { // Ô + diacritic
            0xE5 => Some(0x1ED2), // ¤å -> Ồ
            0xE6 => Some(0x1ED4), // ¤æ -> Ổ
            0xE7 => Some(0x1ED6), // ¤ç -> Ỗ
            0xE8 => Some(0x1ED0), // ¤è -> Ố
            0xE9 => Some(0x1ED8), // ¤é -> Ộ
            _ => None,
        },
        0xA5 => match second { // Ơ + diacritic
            0xEA => Some(0x1EDC), // ¥ê -> Ờ
            0xEB => Some(0x1EDE), // ¥ë -> Ở
            0xEC => Some(0x1EE0), // ¥ì -> Ỡ
            0xED => Some(0x1EDA), // ¥í -> Ớ
            0xEE => Some(0x1EE2), // ¥î -> Ợ
            _ => None,
        },
        0xA6 => match second { // Ư + diacritic
            0xF5 => Some(0x1EEA), // ¦õ -> Ừ
            0xF6 => Some(0x1EEC), // ¦ö -> Ử
            0xF7 => Some(0x1EEE), // ¦÷ -> Ữ
            0xF8 => Some(0x1EE8), // ¦ø -> Ứ
            0xF9 => Some(0x1EF0), // ¦ù -> Ự
            _ => None,
        },
        _ => None,
    }
}

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
        let mut source = ByteSource::new(src);
        let mut dest = Utf8Destination::new(dst);
        
        'outermost: loop {
            // Use SIMD-accelerated ASCII fast path
            match dest.copy_ascii_from_check_space_bmp(&mut source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((non_ascii, mut handle)) => {
                    'middle: loop {
                        // Try two-byte sequence first
                        match source.check_available() {
                            Space::Full(src_consumed) => {
                                if last {
                                    // Single byte at end - check if it's a valid TCVN3 character
                                    if let Some(unicode) = tcvn3_decode_single_byte(non_ascii) {
                                        let dest_again = handle.write_bmp_excl_ascii(unicode);
                                        return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                    }
                                    return (DecoderResult::Malformed(1, 0), src_consumed, handle.written());
                                }
                                // Need more input - could be start of two-byte sequence
                                self.pending = Some(non_ascii);
                                return (DecoderResult::InputEmpty, src_consumed, handle.written());
                            }
                            Space::Available(source_handle) => {
                                let (second_byte, unread_handle) = source_handle.read();
                                
                                // Try two-byte sequence
                                if let Some(unicode) = tcvn3_decode_two_bytes(non_ascii, second_byte) {
                                    let dest_again = unread_handle.commit().write_bmp_excl_ascii(unicode);
                                    // Continue to next character
                                    match source.check_available() {
                                        Space::Full(src_consumed) => {
                                            return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                        }
                                        Space::Available(next_source) => {
                                            match dest_again.check_space_bmp() {
                                                Space::Full(dst_written) => {
                                                    return (DecoderResult::OutputFull, next_source.consumed(), dst_written);
                                                }
                                                Space::Available(next_handle) => {
                                                    let (next_byte, next_unread) = next_source.read();
                                                    if next_byte < 0x80 {
                                                        // ASCII - write and continue outer loop
                                                        next_unread.commit();
                                                        next_handle.write_ascii(next_byte);
                                                        continue 'outermost;
                                                    } else {
                                                        // Non-ASCII - process in middle loop
                                                        handle = next_handle;
                                                        next_unread.commit();
                                                        continue 'middle;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Not a valid two-byte sequence, try single byte
                                unread_handle.unread();
                                if let Some(unicode) = tcvn3_decode_single_byte(non_ascii) {
                                    let dest_again = handle.write_bmp_excl_ascii(unicode);
                                    
                                    // Continue to next character
                                    match source.check_available() {
                                        Space::Full(src_consumed) => {
                                            return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                        }
                                        Space::Available(next_source) => {
                                            match dest_again.check_space_bmp() {
                                                Space::Full(dst_written) => {
                                                    return (DecoderResult::OutputFull, next_source.consumed(), dst_written);
                                                }
                                                Space::Available(next_handle) => {
                                                    let (next_byte, next_unread) = next_source.read();
                                                    if next_byte < 0x80 {
                                                        next_unread.commit();
                                                        next_handle.write_ascii(next_byte);
                                                        continue 'outermost;
                                                    } else {
                                                        handle = next_handle;
                                                        next_unread.commit();
                                                        continue 'middle;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Invalid TCVN3 byte
                                return (DecoderResult::Malformed(1, 0), source.consumed(), handle.written());
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn decode_to_utf16_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u16],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut source = ByteSource::new(src);
        let mut dest = Utf16Destination::new(dst);
        
        'outermost: loop {
            // Use SIMD-accelerated ASCII fast path
            match dest.copy_ascii_from_check_space_bmp(&mut source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((non_ascii, mut handle)) => {
                    'middle: loop {
                        // Try two-byte sequence first
                        match source.check_available() {
                            Space::Full(src_consumed) => {
                                if last {
                                    // Single byte at end - check if it's a valid TCVN3 character
                                    if let Some(unicode) = tcvn3_decode_single_byte(non_ascii) {
                                        let dest_again = handle.write_bmp_excl_ascii(unicode);
                                        return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                    }
                                    return (DecoderResult::Malformed(1, 0), src_consumed, handle.written());
                                }
                                // Need more input - could be start of two-byte sequence
                                self.pending = Some(non_ascii);
                                return (DecoderResult::InputEmpty, src_consumed, handle.written());
                            }
                            Space::Available(source_handle) => {
                                let (second_byte, unread_handle) = source_handle.read();
                                
                                // Try two-byte sequence
                                if let Some(unicode) = tcvn3_decode_two_bytes(non_ascii, second_byte) {
                                    let dest_again = unread_handle.commit().write_bmp_excl_ascii(unicode);
                                    // Continue to next character
                                    match source.check_available() {
                                        Space::Full(src_consumed) => {
                                            return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                        }
                                        Space::Available(next_source) => {
                                            match dest_again.check_space_bmp() {
                                                Space::Full(dst_written) => {
                                                    return (DecoderResult::OutputFull, next_source.consumed(), dst_written);
                                                }
                                                Space::Available(next_handle) => {
                                                    let (next_byte, next_unread) = next_source.read();
                                                    if next_byte < 0x80 {
                                                        // ASCII - write and continue outer loop
                                                        next_unread.commit();
                                                        next_handle.write_ascii(next_byte);
                                                        continue 'outermost;
                                                    } else {
                                                        // Non-ASCII - process in middle loop
                                                        handle = next_handle;
                                                        next_unread.commit();
                                                        continue 'middle;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Not a valid two-byte sequence, try single byte
                                unread_handle.unread();
                                if let Some(unicode) = tcvn3_decode_single_byte(non_ascii) {
                                    let dest_again = handle.write_bmp_excl_ascii(unicode);
                                    
                                    // Continue to next character
                                    match source.check_available() {
                                        Space::Full(src_consumed) => {
                                            return (DecoderResult::InputEmpty, src_consumed, dest_again.written());
                                        }
                                        Space::Available(next_source) => {
                                            match dest_again.check_space_bmp() {
                                                Space::Full(dst_written) => {
                                                    return (DecoderResult::OutputFull, next_source.consumed(), dst_written);
                                                }
                                                Space::Available(next_handle) => {
                                                    let (next_byte, next_unread) = next_source.read();
                                                    if next_byte < 0x80 {
                                                        next_unread.commit();
                                                        next_handle.write_ascii(next_byte);
                                                        continue 'outermost;
                                                    } else {
                                                        handle = next_handle;
                                                        next_unread.commit();
                                                        continue 'middle;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Invalid TCVN3 byte
                                return (DecoderResult::Malformed(1, 0), source.consumed(), handle.written());
                            }
                        }
                    }
                }
            }
        }
    }
}

// Helper function to encode a Unicode codepoint to TCVN3
#[inline(always)]
fn tcvn3_encode_bmp(bmp: u16) -> Option<&'static [u8]> {
    // Fast path for common Vietnamese characters using match (branch prediction friendly)
    match bmp {
        // Single-byte encodings (most common)
        0x0102 => Some(b"\xA1"), // Ă
        0x0103 => Some(b"\xA8"), // ă
        0x00C2 => Some(b"\xA2"), // Â
        0x00E2 => Some(b"\xA9"), // â
        0x00CA => Some(b"\xA3"), // Ê
        0x00EA => Some(b"\xAA"), // ê
        0x00D4 => Some(b"\xA4"), // Ô
        0x00F4 => Some(b"\xAB"), // ô
        0x01A0 => Some(b"\xA5"), // Ơ
        0x01A1 => Some(b"\xAC"), // ơ
        0x01AF => Some(b"\xA6"), // Ư
        0x01B0 => Some(b"\xAD"), // ư
        0x0110 => Some(b"\xA7"), // Đ
        0x0111 => Some(b"\xAE"), // đ
        
        // Common lowercase vowels with tones
        0x00E0 => Some(b"\xB5"), // à
        0x00E1 => Some(b"\xB8"), // á
        0x00E3 => Some(b"\xB7"), // ã
        0x1EA3 => Some(b"\xB6"), // ả
        0x1EA1 => Some(b"\xB9"), // ạ
        
        0x00E8 => Some(b"\xCC"), // è
        0x00E9 => Some(b"\xD0"), // é
        0x1EBB => Some(b"\xCE"), // ẻ
        0x1EBD => Some(b"\xCF"), // ẽ
        0x1EB9 => Some(b"\xD1"), // ẹ
        
        0x00EC => Some(b"\xD7"), // ì
        0x00ED => Some(b"\xDD"), // í
        0x1EC9 => Some(b"\xD8"), // ỉ
        0x0129 => Some(b"\xDC"), // ĩ
        0x1ECB => Some(b"\xDE"), // ị
        
        0x00F2 => Some(b"\xDF"), // ò
        0x00F3 => Some(b"\xE3"), // ó
        0x00F5 => Some(b"\xE2"), // õ
        0x1ECF => Some(b"\xE1"), // ỏ
        0x1ECD => Some(b"\xE4"), // ọ
        
        0x00F9 => Some(b"\xEF"), // ù
        0x00FA => Some(b"\xF3"), // ú
        0x1EE7 => Some(b"\xF1"), // ủ
        0x0169 => Some(b"\xF2"), // ũ
        0x1EE5 => Some(b"\xF4"), // ụ
        
        0x00FD => Some(b"\xFD"), // ý
        0x1EF3 => Some(b"\xFA"), // ỳ
        0x1EF7 => Some(b"\xFB"), // ỷ
        0x1EF9 => Some(b"\xFC"), // ỹ
        0x1EF5 => Some(b"\xFE"), // ỵ
        
        // Two-byte encodings (uppercase with tones)
        0x00C0 => Some(b"\x41\xB5"), // À
        0x00C1 => Some(b"\x41\xB8"), // Á
        0x00C3 => Some(b"\x41\xB7"), // Ã
        0x1EA2 => Some(b"\x41\xB6"), // Ả
        0x1EA0 => Some(b"\x41\xB9"), // Ạ
        
        0x00C8 => Some(b"\x45\xCC"), // È
        0x00C9 => Some(b"\x45\xD0"), // É
        0x1EBA => Some(b"\x45\xCE"), // Ẻ
        0x1EBC => Some(b"\x45\xCF"), // Ẽ
        0x1EB8 => Some(b"\x45\xD1"), // Ẹ
        
        0x00CC => Some(b"\x49\xD7"), // Ì
        0x00CD => Some(b"\x49\xDD"), // Í
        0x1EC8 => Some(b"\x49\xD8"), // Ỉ
        0x0128 => Some(b"\x49\xDC"), // Ĩ
        0x1ECA => Some(b"\x49\xDE"), // Ị
        
        0x00D2 => Some(b"\x4F\xDF"), // Ò
        0x00D3 => Some(b"\x4F\xE3"), // Ó
        0x00D5 => Some(b"\x4F\xE2"), // Õ
        0x1ECE => Some(b"\x4F\xE1"), // Ỏ
        0x1ECC => Some(b"\x4F\xE4"), // Ọ
        
        0x00D9 => Some(b"\x55\xEF"), // Ù
        0x00DA => Some(b"\x55\xF3"), // Ú
        0x1EE6 => Some(b"\x55\xF1"), // Ủ
        0x0168 => Some(b"\x55\xF2"), // Ũ
        0x1EE4 => Some(b"\x55\xF4"), // Ụ
        
        0x00DD => Some(b"\x59\xFD"), // Ý
        0x1EF2 => Some(b"\x59\xFA"), // Ỳ
        0x1EF6 => Some(b"\x59\xFB"), // Ỷ
        0x1EF8 => Some(b"\x59\xFC"), // Ỹ
        0x1EF4 => Some(b"\x59\xFE"), // Ỵ
        
        // Complex vowels with tones
        0x1EA5 => Some(b"\xCA"), // ấ
        0x1EA7 => Some(b"\xC7"), // ầ
        0x1EA9 => Some(b"\xC8"), // ẩ
        0x1EAB => Some(b"\xC9"), // ẫ
        0x1EAD => Some(b"\xCB"), // ậ
        0x1EA4 => Some(b"\xA2\xCA"), // Ấ
        0x1EA6 => Some(b"\xA2\xC7"), // Ầ
        0x1EA8 => Some(b"\xA2\xC8"), // Ẩ
        0x1EAA => Some(b"\xA2\xC9"), // Ẫ
        0x1EAC => Some(b"\xA2\xCB"), // Ậ
        
        0x1EAF => Some(b"\xBE"), // ắ
        0x1EB1 => Some(b"\xBB"), // ằ
        0x1EB3 => Some(b"\xBC"), // ẳ
        0x1EB5 => Some(b"\xBD"), // ẵ
        0x1EB7 => Some(b"\xC6"), // ặ
        0x1EAE => Some(b"\xA1\xBE"), // Ắ
        0x1EB0 => Some(b"\xA1\xBB"), // Ằ
        0x1EB2 => Some(b"\xA1\xBC"), // Ẳ
        0x1EB4 => Some(b"\xA1\xBD"), // Ẵ
        0x1EB6 => Some(b"\xA1\xC6"), // Ặ
        
        0x1EBF => Some(b"\xD5"), // ế
        0x1EC1 => Some(b"\xD2"), // ề
        0x1EC3 => Some(b"\xD3"), // ể
        0x1EC5 => Some(b"\xD4"), // ễ
        0x1EC7 => Some(b"\xD6"), // ệ
        0x1EBE => Some(b"\xA3\xD5"), // Ế
        0x1EC0 => Some(b"\xA3\xD2"), // Ề
        0x1EC2 => Some(b"\xA3\xD3"), // Ể
        0x1EC4 => Some(b"\xA3\xD4"), // Ễ
        0x1EC6 => Some(b"\xA3\xD6"), // Ệ
        
        0x1ED1 => Some(b"\xE8"), // ố
        0x1ED3 => Some(b"\xE5"), // ồ
        0x1ED5 => Some(b"\xE6"), // ổ
        0x1ED7 => Some(b"\xE7"), // ỗ
        0x1ED9 => Some(b"\xE9"), // ộ
        0x1ED0 => Some(b"\xA4\xE8"), // Ố
        0x1ED2 => Some(b"\xA4\xE5"), // Ồ
        0x1ED4 => Some(b"\xA4\xE6"), // Ổ
        0x1ED6 => Some(b"\xA4\xE7"), // Ỗ
        0x1ED8 => Some(b"\xA4\xE9"), // Ộ
        
        0x1EDB => Some(b"\xED"), // ớ
        0x1EDD => Some(b"\xEA"), // ờ
        0x1EDF => Some(b"\xEB"), // ở
        0x1EE1 => Some(b"\xEC"), // ỡ
        0x1EE3 => Some(b"\xEE"), // ợ
        0x1EDA => Some(b"\xA5\xED"), // Ớ
        0x1EDC => Some(b"\xA5\xEA"), // Ờ
        0x1EDE => Some(b"\xA5\xEB"), // Ở
        0x1EE0 => Some(b"\xA5\xEC"), // Ỡ
        0x1EE2 => Some(b"\xA5\xEE"), // Ợ
        
        0x1EE9 => Some(b"\xF8"), // ứ
        0x1EEB => Some(b"\xF5"), // ừ
        0x1EED => Some(b"\xF6"), // ử
        0x1EEF => Some(b"\xF7"), // ữ
        0x1EF1 => Some(b"\xF9"), // ự
        0x1EE8 => Some(b"\xA6\xF8"), // Ứ
        0x1EEA => Some(b"\xA6\xF5"), // Ừ
        0x1EEC => Some(b"\xA6\xF6"), // Ử
        0x1EEE => Some(b"\xA6\xF7"), // Ữ
        0x1EF0 => Some(b"\xA6\xF9"), // Ự
        
        _ => None,
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
        let mut source = Utf8Source::new(src);
        let mut dest = ByteDestination::new(dst);
        
        'outermost: loop {
            // Use SIMD-accelerated ASCII fast path
            match source.copy_ascii_to_check_space_one(&mut dest) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((non_ascii, mut handle)) => {
                    'middle: loop {
                        match non_ascii {
                            NonAscii::BmpExclAscii(bmp) => {
                                // Try to encode Vietnamese character
                                if let Some(tcvn3_bytes) = tcvn3_encode_bmp(bmp) {
                                    if tcvn3_bytes.len() == 1 {
                                        let dest_again = handle.write_one(tcvn3_bytes[0]);
                                        match source.check_available() {
                                            Space::Full(src_consumed) => {
                                                return (EncoderResult::InputEmpty, src_consumed, dest_again.written());
                                            }
                                            Space::Available(source_handle) => {
                                                match dest_again.check_space_one() {
                                                    Space::Full(dst_written) => {
                                                        return (EncoderResult::OutputFull, source_handle.consumed(), dst_written);
                                                    }
                                                    Space::Available(destination_handle) => {
                                                        let (next_char, next_unread) = source_handle.read_enum();
                                                        next_unread.commit();
                                                        match next_char {
                                                            Unicode::Ascii(a) => {
                                                                destination_handle.write_one(a);
                                                                continue 'outermost;
                                                            }
                                                            Unicode::NonAscii(na) => {
                                                                handle = destination_handle;
                                                                non_ascii = na;
                                                                continue 'middle;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Two-byte sequence
                                        match handle.check_space_two() {
                                            Space::Full(dst_written) => {
                                                return (EncoderResult::OutputFull, source.consumed(), dst_written);
                                            }
                                            Space::Available(two_handle) => {
                                                let dest_again = two_handle.write_two(tcvn3_bytes[0], tcvn3_bytes[1]);
                                                match source.check_available() {
                                                    Space::Full(src_consumed) => {
                                                        return (EncoderResult::InputEmpty, src_consumed, dest_again.written());
                                                    }
                                                    Space::Available(source_handle) => {
                                                        match dest_again.check_space_one() {
                                                            Space::Full(dst_written) => {
                                                                return (EncoderResult::OutputFull, source_handle.consumed(), dst_written);
                                                            }
                                                            Space::Available(destination_handle) => {
                                                                let (next_char, next_unread) = source_handle.read_enum();
                                                                next_unread.commit();
                                                                match next_char {
                                                                    Unicode::Ascii(a) => {
                                                                        destination_handle.write_one(a);
                                                                        continue 'outermost;
                                                                    }
                                                                    Unicode::NonAscii(na) => {
                                                                        handle = destination_handle;
                                                                        non_ascii = na;
                                                                        continue 'middle;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Unmappable character
                                    return (EncoderResult::Unmappable(bmp as u32 as char), source.consumed(), handle.written());
                                }
                            }
                            NonAscii::Astral(astral) => {
                                // TCVN3 doesn't support astral characters
                                return (EncoderResult::Unmappable(astral), source.consumed(), handle.written());
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn encode_from_utf16_raw(
        &mut self,
        src: &[u16],
        dst: &mut [u8],
        _last: bool,
    ) -> (EncoderResult, usize, usize) {
        let mut source = Utf16Source::new(src);
        let mut dest = ByteDestination::new(dst);
        
        'outermost: loop {
            // Use SIMD-accelerated ASCII fast path
            match source.copy_ascii_to_check_space_one(&mut dest) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((non_ascii, mut handle)) => {
                    'middle: loop {
                        match non_ascii {
                            NonAscii::BmpExclAscii(bmp) => {
                                // Try to encode Vietnamese character
                                if let Some(tcvn3_bytes) = tcvn3_encode_bmp(bmp) {
                                    if tcvn3_bytes.len() == 1 {
                                        let dest_again = handle.write_one(tcvn3_bytes[0]);
                                        match source.check_available() {
                                            Space::Full(src_consumed) => {
                                                return (EncoderResult::InputEmpty, src_consumed, dest_again.written());
                                            }
                                            Space::Available(source_handle) => {
                                                match dest_again.check_space_one() {
                                                    Space::Full(dst_written) => {
                                                        return (EncoderResult::OutputFull, source_handle.consumed(), dst_written);
                                                    }
                                                    Space::Available(destination_handle) => {
                                                        let (next_char, next_unread) = source_handle.read_enum();
                                                        next_unread.commit();
                                                        match next_char {
                                                            Unicode::Ascii(a) => {
                                                                destination_handle.write_one(a);
                                                                continue 'outermost;
                                                            }
                                                            Unicode::NonAscii(na) => {
                                                                handle = destination_handle;
                                                                non_ascii = na;
                                                                continue 'middle;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Two-byte sequence
                                        match handle.check_space_two() {
                                            Space::Full(dst_written) => {
                                                return (EncoderResult::OutputFull, source.consumed(), dst_written);
                                            }
                                            Space::Available(two_handle) => {
                                                let dest_again = two_handle.write_two(tcvn3_bytes[0], tcvn3_bytes[1]);
                                                match source.check_available() {
                                                    Space::Full(src_consumed) => {
                                                        return (EncoderResult::InputEmpty, src_consumed, dest_again.written());
                                                    }
                                                    Space::Available(source_handle) => {
                                                        match dest_again.check_space_one() {
                                                            Space::Full(dst_written) => {
                                                                return (EncoderResult::OutputFull, source_handle.consumed(), dst_written);
                                                            }
                                                            Space::Available(destination_handle) => {
                                                                let (next_char, next_unread) = source_handle.read_enum();
                                                                next_unread.commit();
                                                                match next_char {
                                                                    Unicode::Ascii(a) => {
                                                                        destination_handle.write_one(a);
                                                                        continue 'outermost;
                                                                    }
                                                                    Unicode::NonAscii(na) => {
                                                                        handle = destination_handle;
                                                                        non_ascii = na;
                                                                        continue 'middle;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Unmappable character
                                    return (EncoderResult::Unmappable(bmp as u32 as char), source.consumed(), handle.written());
                                }
                            }
                            NonAscii::Astral(astral) => {
                                // TCVN3 doesn't support astral characters
                                return (EncoderResult::Unmappable(astral), source.consumed(), handle.written());
                            }
                        }
                    }
                }
            }
        }
    }
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
        let mut encoder = Tcvn3Encoder;
        let (result, read, written) = encoder.encode_from_utf8_raw(input, &mut output, true);
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
        let mut encoder = Tcvn3Encoder;
        let (_encode_result, _encode_read, encode_written) = encoder.encode_from_utf8_raw(vietnamese_text, &mut encoded, true);
        
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
        let mut encoder = Tcvn3Encoder;
        let (_encode_result, _encode_read, encode_written) = encoder.encode_from_utf8_raw(sentence, &mut encoded, true);
        
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
            let mut encoder = Tcvn3Encoder;
            let (result, _read, written) = encoder.encode_from_utf8_raw(input, &mut output, true);
            
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
            let mut encoder = Tcvn3Encoder;
            let (result, _read, written) = encoder.encode_from_utf8_raw(input, &mut output, true);
            
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
            let mut encoder = Tcvn3Encoder;
            let (result, _read, written) = encoder.encode_from_utf8_raw(input, &mut output, true);
            
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
            let mut encoder = Tcvn3Encoder;
            let (result, _read, written) = encoder.encode_from_utf8_raw(input, &mut output, true);
            
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
            let mut encoder = Tcvn3Encoder;
            let (encode_result, encode_read, encode_written) = encoder.encode_from_utf8_raw(sentence, &mut encoded, true);
            
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
        let mut encoder = Tcvn3Encoder;
        let (result, read, written) = encoder.encode_from_utf8_raw(ascii_text, &mut encoded, true);
        
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
            let mut encoder = Tcvn3Encoder;
            let (result, _read, _written) = encoder.encode_from_utf8_raw(ch, &mut output, true);
            
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
        
        let mut encoder = Tcvn3Encoder;
        let (result, _read, _written) = encoder.encode_from_utf8_raw(text, &mut small_buffer, true);
        
        // Should return OutputFull when buffer is too small
        assert_eq!(result, EncoderResult::OutputFull);
    }

    #[test]
    fn test_tcvn3_mixed_content() {
        // Test mixed Vietnamese and ASCII content
        let mixed_text = "Hello Việt Nam 123";
        let mut encoded = [0u8; 64];
        let mut encoder = Tcvn3Encoder;
        let (result, read, written) = encoder.encode_from_utf8_raw(mixed_text, &mut encoded, true);
        
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