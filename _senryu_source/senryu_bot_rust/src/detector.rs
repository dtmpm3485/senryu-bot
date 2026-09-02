
use anyhow::{Context, Result};
use lindera::{
    dictionary::load_dictionary,
    mode::Mode,
    segmenter::Segmenter,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::{borrow::Cow, sync::Arc};
use unicode_normalization::UnicodeNormalization;

static DISCORD_TOKEN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<@!?\d+>|<#\d+>|<@&\d+>|<a?:\w+:\d+>|https?://\S+").unwrap()
});
static FENCED_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)```.*?```").unwrap());
static INLINE_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"`[^`]+`").unwrap());
static SPOILER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\|\|.+?\|\|").unwrap());

#[derive(Clone, Debug)]
struct LexToken {
    surface: String,
    reading: String,
    pos: String,
    pos1: String,
    conjugation_form: String,
}

#[derive(Clone)]
pub struct Detector {
    segmenter: Arc<Segmenter>,
}

impl Detector {
    pub fn new() -> Result<Self> {
        let dictionary =
            load_dictionary("embedded://unidic").context("failed to load embedded UniDic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        Ok(Self {
            segmenter: Arc::new(segmenter),
        })
    }

    pub fn find_575(&self, text: &str) -> Result<Option<[String; 3]>> {
        self.find(text, &[5, 7, 5])
    }

    pub fn find(&self, text: &str, pattern: &[usize]) -> Result<Option<[String; 3]>> {
        if pattern != [5, 7, 5] {
            anyhow::bail!("this bot detector currently exposes the 5-7-5 pattern");
        }

        let normalized = normalize_for_tokenizer(text);
        let lines: Vec<&str> = normalized.lines().collect();

        // The original bot rejects a detected verse if it crosses a newline.
        // Running the detector independently per line makes that invariant explicit.
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(v) = self.find_in_single_line(line, pattern)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    fn find_in_single_line(
        &self,
        text: &str,
        pattern: &[usize],
    ) -> Result<Option<[String; 3]>> {
        let mut raw = self
            .segmenter
            .segment(Cow::Borrowed(text))
            .context("Japanese tokenization failed")?;

        let mut tokens = Vec::<LexToken>::new();
        for tok in raw.iter_mut() {
            let surface = tok.surface.to_string();
            if surface.trim().is_empty() {
                continue;
            }

            let pos = tok.get("part_of_speech").map(str::to_owned).unwrap_or_default();
            let pos1 = tok
                .get("part_of_speech_subcategory_1")
                .map(str::to_owned)
                .unwrap_or_default();
            let conjugation_form = tok
                .get("conjugation_form")
                .map(str::to_owned)
                .unwrap_or_default();

            if is_ignored_pos(&pos, &pos1) {
                continue;
            }

            // go-haiku uses a katakana surface directly; otherwise it prefers
            // the dictionary pronunciation. Unknown non-katakana tokens then
            // naturally break the current candidate.
            let reading = if is_katakana_word(&surface) {
                surface.clone()
            } else {
                let phonological = tok.get("phonological_surface_form").map(str::to_owned);
                match phonological {
                    Some(v) if v != "*" && !v.is_empty() => v,
                    _ => tok
                        .get("reading")
                        .map(str::to_owned)
                        .filter(|v| v != "*" && !v.is_empty())
                        .unwrap_or_default(),
                }
            };

            tokens.push(LexToken {
                surface,
                reading,
                pos,
                pos1,
                conjugation_form,
            });
        }

        // Search from every possible token boundary. This mirrors go-haiku's
        // reset/retry behavior while keeping the implementation straightforward.
        for start in 0..tokens.len() {
            if !is_word_start(&tokens[start]) {
                continue;
            }

            let mut line_index = 0usize;
            let mut remaining = pattern[0] as isize;
            let mut ambiguous = 0isize;
            let mut parts = [String::new(), String::new(), String::new()];

            for token in tokens.iter().skip(start) {
                if !is_valid_reading(&token.reading) || is_digit_token(&token.surface) {
                    break;
                }

                // At the beginning of each 5/7/5 segment, go-haiku requires a
                // token category that can lead a phrase.
                if remaining == pattern[line_index] as isize && !is_word_start(token) {
                    break;
                }

                ambiguous += token
                    .reading
                    .chars()
                    .filter(|c| matches!(c, 'ッ' | 'ー'))
                    .count() as isize;
                remaining -= mora_count(&token.reading) as isize;
                parts[line_index].push_str(&token.surface);

                if remaining >= 0 && (remaining == 0 || remaining + ambiguous == 0) {
                    line_index += 1;
                    if line_index == pattern.len() {
                        // Find() in the upstream implementation validates the
                        // grammatical end only at the end of the whole verse.
                        if is_sentence_end(token) {
                            return Ok(Some(parts));
                        }
                        break;
                    }
                    remaining = pattern[line_index] as isize;
                } else if remaining < 0 {
                    break;
                }
            }
        }

        Ok(None)
    }

}

pub fn contains_discord_tokens(text: &str) -> bool {
    DISCORD_TOKEN_RE.is_match(text)
}

pub fn strip_code_blocks(text: &str) -> String {
    let no_fenced = FENCED_CODE_RE.replace_all(text, "");
    INLINE_CODE_RE.replace_all(&no_fenced, "").into_owned()
}

pub fn contains_spoiler(text: &str) -> bool {
    SPOILER_RE.is_match(text)
}

pub fn strip_spoiler_markers(text: &str) -> String {
    text.replace("||", "")
}

pub fn is_japanese_rich(text: &str) -> bool {
    let mut total = 0usize;
    let mut jp = 0usize;

    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        if is_hiragana(ch)
            || is_katakana(ch)
            || is_cjk(ch)
            || matches!(ch, 'ー' | '・')
        {
            jp += 1;
        }
    }

    total != 0 && (jp as f64 / total as f64) >= 0.5
}

pub fn mora_count(reading: &str) -> usize {
    reading
        .chars()
        .filter(|c| !matches!(c, 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ' | 'ャ' | 'ュ' | 'ョ'))
        .count()
}

fn normalize_for_tokenizer(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    nfkc.chars()
        .map(|c| {
            if matches!(c, '[' | ']' | '「' | '」' | '『' | '』' | '、' | '。' | '？' | '！') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn is_ignored_pos(pos: &str, pos1: &str) -> bool {
    pos == "空白" || pos == "補助記号" || (pos == "記号" && pos1 == "空白")
}

// Equivalent to go-haiku's isWord, adapted to named UniDic fields.
fn is_word_start(t: &LexToken) -> bool {
    if t.pos != "名詞" && t.pos1 == "非自立" {
        return false;
    }

    if matches!(
        t.pos.as_str(),
        "名詞" | "形容詞" | "形容動詞" | "副詞" | "連体詞" | "接続詞" | "感動詞" | "接頭詞" | "フィラー"
    ) && t.pos1 != "接尾" {
        return true;
    }
    if t.pos == "接頭辞" || (t.pos == "接続詞" && t.pos1 == "名詞接続") {
        return false;
    }
    if t.pos == "形状詞" && t.pos1 != "助動詞語幹" {
        return true;
    }
    if t.pos == "代名詞" {
        return true;
    }
    if t.pos == "記号" && t.pos1 == "一般" {
        return true;
    }
    if t.pos == "助詞"
        && !matches!(
            t.pos1.as_str(),
            "副助詞" | "準体助詞" | "終助詞" | "係助詞" | "格助詞" | "接続助詞" | "連体化" | "副助詞／並立助詞／終助詞"
        )
    {
        return true;
    }
    if t.pos == "動詞" && t.pos1 != "接尾" && t.pos1 != "非自立" {
        return true;
    }
    matches!(t.pos.as_str(), "カスタム人名" | "カスタム名詞")
}

fn is_sentence_end(t: &LexToken) -> bool {
    if t.pos == "接頭辞" {
        // Upstream rejects the honorific prefix 御 as a sentence end.
        return t.surface != "御";
    }
    if t.pos1 == "非自立" {
        if matches!(t.pos.as_str(), "名詞" | "動詞") {
            return true;
        }
        if t.reading == "ノ" {
            return true;
        }
        return false;
    }
    if t.conjugation_form.starts_with("未然形") {
        return false;
    }
    true
}

fn is_valid_reading(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| ('ァ'..='ヾ').contains(&c))
}

fn is_katakana_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| ('ァ'..='ヶ').contains(&c) || c == 'ー')
}

fn is_digit_token(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
}

fn is_hiragana(c: char) -> bool {
    ('\u{3040}'..='\u{309f}').contains(&c)
}

fn is_katakana(c: char) -> bool {
    ('\u{30a0}'..='\u{30ff}').contains(&c)
}

fn is_cjk(c: char) -> bool {
    ('\u{3400}'..='\u{4dbf}').contains(&c)
        || ('\u{4e00}'..='\u{9fff}').contains(&c)
        || ('\u{f900}'..='\u{faff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mora_rules() {
        assert_eq!(mora_count("キョウ"), 2);
        assert_eq!(mora_count("ガッコウ"), 4);
        assert_eq!(mora_count("ニッポン"), 4);
        assert_eq!(mora_count("コーヒー"), 4);
        assert_eq!(mora_count("シャシン"), 3);
    }

    #[test]
    fn discord_tokens_are_rejected() {
        assert!(contains_discord_tokens("<@123456> こんにちは"));
        assert!(contains_discord_tokens("https://example.com"));
        assert!(contains_discord_tokens("<:foo:123456>"));
        assert!(!contains_discord_tokens("普通の日本語です"));
    }

    #[test]
    fn code_and_spoilers() {
        assert_eq!(strip_code_blocks("あ`code`い"), "あい");
        assert!(contains_spoiler("||秘密||"));
        assert_eq!(strip_spoiler_markers("||秘密||"), "秘密");
    }

    #[test]
    fn japanese_ratio() {
        assert!(is_japanese_rich("今日は学校です"));
        assert!(!is_japanese_rich("hello world"));
    }
}
