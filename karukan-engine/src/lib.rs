pub mod dict;
pub mod kana;
pub mod kanji;
pub mod learning;
pub mod rewriter;
pub mod romaji;

pub use dict::{Candidate as DictCandidate, DictEntry, Dictionary, LookupResult};
pub use kana::{
    contains_kana, hiragana_to_katakana, is_pure_full_katakana, is_pure_hiragana,
    katakana_to_hiragana, normalize_nfkc, to_half_width,
};
pub use kanji::{Backend, KanaKanjiConverter};
pub use learning::LearningCache;
pub use rewriter::{
    AlphabetRewriter, EmojiRewriter, HalfWidthKatakanaRewriter, RewriteOutput, Rewriter,
    RewriterChain, SpecialConversionRewriter, SymbolRewriter, description as symbol_description,
};
pub use romaji::{BackspaceResult, ConversionEvent, RomajiConverter};

#[cfg(target_os = "macos")]
fn performance_core_count_from_sysctl(
    status: libc::c_int,
    value_len: usize,
    value: libc::c_int,
) -> Option<u32> {
    (status == 0 && value_len == std::mem::size_of::<libc::c_int>() && value > 0)
        .then_some(value as u32)
}

/// Return the number of performance cores reported by macOS.
#[cfg(target_os = "macos")]
pub fn performance_core_count() -> Option<u32> {
    let name = b"hw.perflevel0.physicalcpu\0";
    let mut value: libc::c_int = 0;
    let mut value_len = std::mem::size_of_val(&value);
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr().cast(),
            (&mut value as *mut libc::c_int).cast(),
            &mut value_len,
            std::ptr::null_mut(),
            0,
        )
    };
    performance_core_count_from_sysctl(status, value_len, value)
}

/// Performance-core topology is not available through the macOS sysctl on other platforms.
#[cfg(not(target_os = "macos"))]
pub fn performance_core_count() -> Option<u32> {
    None
}

#[cfg(test)]
mod performance_core_tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn sysctl_response_requires_status_size_and_positive_value() {
        let expected_len = std::mem::size_of::<libc::c_int>();
        assert_eq!(
            performance_core_count_from_sysctl(0, expected_len, 4),
            Some(4)
        );
        assert_eq!(
            performance_core_count_from_sysctl(-1, expected_len, 4),
            None
        );
        assert_eq!(
            performance_core_count_from_sysctl(0, expected_len - 1, 4),
            None
        );
        assert_eq!(performance_core_count_from_sysctl(0, expected_len, 0), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_performance_core_count_is_positive_when_available() {
        if let Some(count) = performance_core_count() {
            assert!(count > 0);
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_has_no_performance_core_count() {
        assert_eq!(performance_core_count(), None);
    }
}
