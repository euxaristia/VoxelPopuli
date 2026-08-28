// Minimal JSON reader for Vibrant Visuals pack files. Hand-rolled to match
// the rest of the tree (see save.rs, java_compat.rs) rather than pull in a
// serde dependency. Accepts the `//` and `/* */` comments and trailing
// commas that real Minecraft JSON is written with.
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    // Object keys keep source order and are searched linearly: pack files
    // hold a handful of keys each, so a map would cost more than it saves.
    Object(Vec<(String, Json)>),
}

#[derive(Debug, PartialEq)]
pub struct JsonError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|n| n as f32)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(fields) => Some(fields),
            _ => None,
        }
    }
}

pub fn parse(source: &str) -> Result<Json, JsonError> {
    let mut p = Parser {
        bytes: source.as_bytes(),
        pos: 0,
        line: 1,
    };
    p.skip_trivia();
    let value = p.value()?;
    p.skip_trivia();
    if p.pos != p.bytes.len() {
        return Err(p.error("trailing data after top-level value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: &str) -> JsonError {
        JsonError {
            line: self.line,
            message: message.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
        }
        Some(b)
    }

    // Whitespace plus the two comment forms Minecraft JSON is written with.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_whitespace() => {
                    self.bump();
                }
                Some(b'/') if self.bytes.get(self.pos + 1) == Some(&b'/') => {
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some(b'/') if self.bytes.get(self.pos + 1) == Some(&b'*') => {
                    self.bump();
                    self.bump();
                    while self.pos < self.bytes.len() {
                        if self.peek() == Some(b'*') && self.bytes.get(self.pos + 1) == Some(&b'/')
                        {
                            self.bump();
                            self.bump();
                            break;
                        }
                        self.bump();
                    }
                }
                _ => return,
            }
        }
    }

    fn expect(&mut self, want: u8) -> Result<(), JsonError> {
        if self.peek() == Some(want) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(&format!("expected {:?}", want as char)))
        }
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b) if b == b'-' || b == b'+' || b.is_ascii_digit() => self.number(),
            Some(_) => Err(self.error("unexpected character")),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            for _ in 0..word.len() {
                self.bump();
            }
            Ok(value)
        } else {
            Err(self.error(&format!("expected {word}")))
        }
    }

    fn object(&mut self) -> Result<Json, JsonError> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some(b'}') {
                self.bump();
                return Ok(Json::Object(fields));
            }
            let key = self.string()?;
            self.skip_trivia();
            self.expect(b':')?;
            self.skip_trivia();
            let value = self.value()?;
            fields.push((key, value));
            self.skip_trivia();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b'}') => {}
                _ => return Err(self.error("expected , or } in object")),
            }
        }
    }

    fn array(&mut self) -> Result<Json, JsonError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Json::Array(items));
            }
            items.push(self.value()?);
            self.skip_trivia();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b']') => {}
                _ => return Err(self.error("expected , or ] in array")),
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let b = self
                .bump()
                .ok_or_else(|| self.error("unterminated string"))?;
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = self
                        .bump()
                        .ok_or_else(|| self.error("unterminated escape"))?;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        _ => return Err(self.error("invalid escape")),
                    }
                }
                _ => {
                    // Copy the whole UTF-8 sequence; the bytes came from a
                    // &str, so the continuation bytes are already valid.
                    let start = self.pos - 1;
                    for _ in 1..utf8_len(b) {
                        self.bump();
                    }
                    out.push_str(std::str::from_utf8(&self.bytes[start..self.pos]).unwrap());
                }
            }
        }
    }

    // Reads the 4 hex digits after \u, joining a surrogate pair when needed.
    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let code = self.hex4()?;
        if !(0xd800..0xdc00).contains(&code) {
            return char::from_u32(code).ok_or_else(|| self.error("invalid \\u escape"));
        }
        if self.peek() != Some(b'\\') {
            return Err(self.error("lone high surrogate"));
        }
        self.bump();
        if self.peek() != Some(b'u') {
            return Err(self.error("lone high surrogate"));
        }
        self.bump();
        let low = self.hex4()?;
        if !(0xdc00..0xe000).contains(&low) {
            return Err(self.error("invalid low surrogate"));
        }
        let scalar = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
        char::from_u32(scalar).ok_or_else(|| self.error("invalid surrogate pair"))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut code = 0u32;
        for _ in 0..4 {
            let b = self
                .bump()
                .ok_or_else(|| self.error("truncated \\u escape"))?;
            let digit = (b as char)
                .to_digit(16)
                .ok_or_else(|| self.error("invalid hex digit"))?;
            code = code * 16 + digit;
        }
        Ok(code)
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if matches!(self.peek(), Some(b'-') | Some(b'+')) {
            self.bump();
        }
        while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some(b'.') {
            self.bump();
            while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'-') | Some(b'+')) {
                self.bump();
            }
            while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
                self.bump();
            }
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .map(Json::Number)
            .ok_or_else(|| self.error("invalid number"))
    }
}

fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_objects_and_arrays() {
        let value = parse(r##"{"a": [1, 2.5, -3e2], "b": {"c": true, "d": null}}"##).unwrap();
        assert_eq!(value.get("a").unwrap().as_array().unwrap().len(), 3);
        assert_eq!(
            value.get("a").unwrap().as_array().unwrap()[2].as_f64(),
            Some(-300.0)
        );
        assert_eq!(value.get("b").unwrap().get("c"), Some(&Json::Bool(true)));
        assert_eq!(value.get("b").unwrap().get("d"), Some(&Json::Null));
    }

    #[test]
    fn accepts_comments_and_trailing_commas() {
        // The manifest.json sample in the Vibrant Visuals docs ships with
        // line comments inside the capabilities array.
        let source = r##"{
            // leading comment
            "capabilities": [
                "pbr", // Vibrant Visuals
                "raytraced",
            ],
            /* block
               comment */
            "n": 1,
        }"##;
        let value = parse(source).unwrap();
        assert_eq!(
            value.get("capabilities").unwrap().as_array().unwrap().len(),
            2
        );
        assert_eq!(value.get("n").unwrap().as_f64(), Some(1.0));
    }

    #[test]
    fn parses_string_escapes_and_surrogate_pairs() {
        let value = parse(r##"{"s": "a\"b\\c\né😀"}"##).unwrap();
        assert_eq!(
            value.get("s").unwrap().as_str(),
            Some("a\"b\\c\n\u{e9}\u{1f600}")
        );
    }

    #[test]
    fn keeps_duplicate_keys_in_source_order() {
        let value = parse(r##"{"k": 1, "k": 2}"##).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 2);
        // get() returns the first, matching the "first wins" behaviour the
        // rest of the loader assumes.
        assert_eq!(value.get("k").unwrap().as_f64(), Some(1.0));
    }

    #[test]
    fn reports_the_line_of_a_syntax_error() {
        let err = parse("{\n  \"a\": 1\n  \"b\": 2\n}").unwrap_err();
        assert_eq!(err.line, 3);
    }

    #[test]
    fn rejects_trailing_data() {
        assert!(parse("{} {}").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn rejects_lone_surrogates() {
        assert!(parse(r##"{"s": "\ud83d"}"##).is_err());
        assert!(parse(r##"{"s": "\ud83dA"}"##).is_err());
    }
}
