//! The Unicode normalisation form of an original file name (SRS-F-002, MD-S-013): the
//! record keeps the exact bytes and says which form they are in, so a name can be compared
//! with the same name from another system (macOS file systems hand names over in NFD).

use filesquid_metadata::import::NameForm;
use unicode_normalization::{is_nfc, is_nfd, is_nfkc, is_nfkd};

/// The form `name` is in: NFC when it is (an ASCII name is in every form), else NFD, NFKC
/// or NFKD in that order, and unnormalised when it is in none of them or not UTF-8. Pure.
pub fn name_form(name: &[u8]) -> NameForm {
    let Ok(text) = std::str::from_utf8(name) else {
        return NameForm::Unnormalised;
    };
    if is_nfc(text) {
        return NameForm::Nfc;
    }
    if is_nfd(text) {
        return NameForm::Nfd;
    }
    if is_nfkc(text) {
        return NameForm::Nfkc;
    }
    if is_nfkd(text) {
        return NameForm::Nfkd;
    }
    NameForm::Unnormalised
}
