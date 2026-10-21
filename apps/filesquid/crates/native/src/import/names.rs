//! The Unicode normalisation form of an original file name (SRS-F-002, MD-S-013): the
//! record keeps the exact bytes and says which form they are in, so a name can be compared
//! with the same name from another system (macOS file systems hand names over in NFD).
//!
//! A name in NFC and NFD at once (pure ASCII, or text without composable characters) is
//! recorded as NFC, as the API records an ASCII upload name; otherwise the first form the name
//! is in, in the order NFD, NFC, NFKC, NFKD; and `unnormalised` when it is in none or is not
//! UTF-8 (orchestrator decision 2026-10-20).

use filesquid_metadata::import::NameForm;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

/// The form `name` is in. Pure.
pub fn name_form(name: &[u8]) -> NameForm {
    let Ok(text) = std::str::from_utf8(name) else {
        return NameForm::Unnormalised;
    };
    let nfc = ComposingNormalizerBorrowed::new_nfc().is_normalized(text);
    let nfd = DecomposingNormalizerBorrowed::new_nfd().is_normalized(text);
    if nfc && nfd {
        return NameForm::Nfc;
    }
    if nfd {
        return NameForm::Nfd;
    }
    if nfc {
        return NameForm::Nfc;
    }
    if ComposingNormalizerBorrowed::new_nfkc().is_normalized(text) {
        return NameForm::Nfkc;
    }
    if DecomposingNormalizerBorrowed::new_nfkd().is_normalized(text) {
        return NameForm::Nfkd;
    }
    NameForm::Unnormalised
}
