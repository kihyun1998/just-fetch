# Choosing the language

How just-fetch picks between English and Korean, and where its words live: `lang::resolve` picks, `lang::set` holds the pick for the run, and `text::Text` holds every sentence just-fetch says on its own.

## Which language

The one `setup` saved in `config.toml` (`language = "ko"` or `"en"`), else the OS language: the first of the OS's preferred languages, from the `sys-locale` crate. Korean when that tag's primary language is `ko`, English for anything else (`kok`, Konkani, stays English). Chosen by the maintainer.

- **Not `LANG`.** On macOS `sys-locale` asks Core Foundation for the preferred languages (`CFLocaleCopyPreferredLanguages`), and on Windows for the display language (`GetUserPreferredUILanguages`); neither reads `LANG`. The machine this was written on had `LANG=en_US.UTF-8` and `ko-KR` first in its preferred languages, and is Korean (read in sys-locale 0.3.2's source).
- **`JUST_FETCH_SYSTEM_LANGUAGE`** stands in for the OS language, as a tag such as `ko-KR`. The tests set it to `en` wherever they start just-fetch, and point `JUST_FETCH_CONFIG_DIR` away from the person's own config, which could hold a language too; without both, the test suite turned Korean on that machine (measured: turning the variable off reddened 15 tests).
- The language is read before anything else, from a config that cannot be read too: a broken or missing file falls back to the OS language, so a run given a path still speaks the right language, and the broken file is reported by whatever reads it next.
- A `language` other than `en` or `ko` counts as none saved, so a typo does not block `setup`, which is where it gets fixed.
- `setup` asks first (`Language` / `언어`, the languages named in themselves, the one in use picked to start with) and saves the answer at once.
- `l` on the pick screen switches the screen, and what the run prints after it, to the other language. It is not saved; saving is `setup`'s. Chosen by the maintainer.

## Where the words live

`text::Text` has one variant for each thing just-fetch says, and `Text::both` gives its English and Korean as a pair, so a sentence cannot be added in one language without the other. Its `Display` uses the run's language; the pick screen renders in its own language through `Text::in_lang`, so `l` redraws everything on it.

- **What is stored is not a sentence.** A refused fetch is `Fetch::Refused(key)`, too old a git is `Prediction::NeedsNewerGit`, a clone that changed during the pick is `Outcome::Changed(Change)`, and a pick-screen row holds a `Text`. Each turns into words only when shown, so `l` can switch them after they were made.
- **What stays a sentence.** A reason built deep inside a git call (just-fetch's own path not found, git printing what it should not) becomes a string where it is made, so it keeps the language the run started in even after `l`. Those paths are rare.
- **What stays as it is.** Paths, clone names and git's own messages pass through untranslated, and so do clap's `--help` and argument errors. Chosen by the maintainer.
- **The tests.** `text`'s tests render one of every variant in both languages and check that the Korean has Hangul and the English none. They also read every other file in `src/` and fail on a literal holding a word that is printed, prompted, raised as an error or turned into a `String` (`println!(`, `with_prompt(`, `Err(`, `.to_string()` and the like), so a message written outside `text.rs` fails whether it is one word or a sentence. A message built some other way, such as through `format!` alone, is not seen. The few such literals that are not said to a person (two git config keys, the `just-fetch:` prefix, a panic message) are listed by name.
- Korean is never pluralised; English counts go through `count`, which says `1 clone` and `2 clones`.
