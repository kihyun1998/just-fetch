use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// A language just-fetch speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ko,
}

impl Lang {
    /// The language `l` switches to.
    pub fn other(self) -> Self {
        match self {
            Lang::En => Lang::Ko,
            Lang::Ko => Lang::En,
        }
    }
}

/// The variable that stands in for the OS language, as a BCP 47 tag such as `ko-KR`.
pub const SYSTEM_LANGUAGE_VAR: &str = "JUST_FETCH_SYSTEM_LANGUAGE";

/// Korean for a tag whose primary language is `ko`, English for anything else.
pub fn from_tag(tag: &str) -> Lang {
    match tag.split(['-', '_']).next() {
        Some(primary) if primary.eq_ignore_ascii_case("ko") => Lang::Ko,
        _ => Lang::En,
    }
}

/// The OS language: `JUST_FETCH_SYSTEM_LANGUAGE`, else the first of the OS's preferred
/// languages.
pub fn system() -> Lang {
    let tag = std::env::var(SYSTEM_LANGUAGE_VAR)
        .ok()
        .or_else(sys_locale::get_locale);
    tag.map_or(Lang::En, |tag| from_tag(&tag))
}

/// The language a run speaks: the one saved in the config, else the OS language.
pub fn resolve(saved: Option<Lang>) -> Lang {
    saved.unwrap_or_else(system)
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The language this run speaks; English until [`set`] says otherwise.
pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Ko,
        _ => Lang::En,
    }
}

/// Makes `lang` the language the rest of this run speaks.
pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_korean_tag_in_any_spelling_is_korean() {
        for tag in ["ko", "ko-KR", "ko_KR.UTF-8", "KO-kr", "ko-Kore-KR"] {
            assert_eq!(from_tag(tag), Lang::Ko, "{tag}");
        }
    }

    #[test]
    fn every_other_tag_is_english() {
        for tag in ["en-US", "ja-JP", "kok-IN", "", "C"] {
            assert_eq!(from_tag(tag), Lang::En, "{tag}");
        }
    }

    #[test]
    fn a_saved_language_wins_over_the_os() {
        assert_eq!(resolve(Some(Lang::Ko)), Lang::Ko);
        assert_eq!(resolve(Some(Lang::En)), Lang::En);
    }
}
