use std::fmt;

/// An input or usage problem. The message never includes file contents.
/// Control and invisible/direction characters (see `must_escape`) are escaped so a path or name
/// cannot break or reverse the line.
/// The process exit code is chosen by the binary: 0, 1, or 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: escape_controls(&message.into()),
        }
    }

    /// Text without the `cwlogs-retentie:` prefix.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Path or name text for errors and warnings.
/// Characters matching `must_escape` are written as `\u{..}`, newline and CR included. Other characters stay.
pub fn escape_controls(text: &str) -> String {
    if !text.chars().any(must_escape) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if must_escape(ch) {
            out.extend(ch.escape_unicode());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Text for an operating-system string (a path or an argument).
/// Valid UTF-8 goes through [`escape_controls`]; each byte that is not valid UTF-8 is
/// written as `\xNN` on Unix, so the result is one line of printable text.
pub fn escape_os_str(text: &std::ffi::OsStr) -> String {
    if let Some(valid) = text.to_str() {
        return escape_controls(valid);
    }
    #[cfg(unix)]
    {
        use std::fmt::Write as _;
        use std::os::unix::ffi::OsStrExt;
        let mut out = String::new();
        for chunk in text.as_bytes().utf8_chunks() {
            out.push_str(&escape_controls(chunk.valid()));
            for byte in chunk.invalid() {
                let _ = write!(out, "\\x{byte:02x}");
            }
        }
        out
    }
    #[cfg(not(unix))]
    {
        escape_controls(&text.to_string_lossy())
    }
}

/// Control, invisible, line-breaking and direction-changing characters.
pub(crate) fn must_escape(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{ad}'
                | '\u{34f}'
                | '\u{61c}'
                | '\u{115f}'
                | '\u{1160}'
                | '\u{17b4}'
                | '\u{17b5}'
                | '\u{180b}'..='\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{2800}'
                | '\u{3164}'
                | '\u{fe00}'..='\u{fe0f}'
                | '\u{feff}'
                | '\u{ffa0}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0000}'..='\u{e007f}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_and_marks_are_escaped_in_labels_and_warnings() {
        for ch in [
            '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}', '\u{200e}', '\u{200f}',
        ] {
            let shown = escape_controls(&format!("a{ch}b"));
            assert_eq!(shown, format!("a\\u{{{:x}}}b", ch as u32));
            assert!(!shown.contains(ch));
            let err = Error::new(format!("p{ch}q: bad"));
            assert!(!err.message().contains(ch));
            let doc =
                crate::parse::parse_document(&format!("f{ch}.json"), b"not json", false).unwrap();
            assert!(
                doc.warnings.iter().all(|w| !w.contains(ch)),
                "{:?}",
                doc.warnings
            );
            assert!(doc.warnings[0].contains(&format!("f\\u{{{:x}}}.json", ch as u32)));
        }
    }

    #[test]
    fn added_invisible_characters_are_escaped() {
        let added = [0x115f, 0x1160, 0x17b4, 0x17b5, 0xffa0]
            .into_iter()
            .chain(0x180b..=0x180d)
            .chain(0xfe00..=0xfe0e)
            .chain(0xfff9..=0xfffb);
        for cp in added {
            let ch = char::from_u32(cp).unwrap();
            assert!(must_escape(ch), "{cp:x}");
            let shown = escape_controls(&format!("a{ch}b"));
            assert_eq!(shown, format!("a\\u{{{cp:x}}}b"));
        }
    }
}
