use super::serde_helpers;
use serde::{Deserialize, Serialize};
use std::{fmt, ops::Range, str::FromStr};
use yansi::{Color, Paint, Style};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SourceLocation {
    pub file: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SecondarySourceLocation {
    pub file: Option<String>,
    pub start: Option<i32>,
    pub end: Option<i32>,
    pub message: Option<String>,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Error" | "error" => Ok(Self::Error),
            "Warning" | "warning" => Ok(Self::Warning),
            "Info" | "info" => Ok(Self::Info),
            s => Err(format!("Invalid severity: {s}")),
        }
    }
}

impl Severity {
    /// Returns `true` if the severity is `Error`.
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// Returns `true` if the severity is `Warning`.
    pub const fn is_warning(&self) -> bool {
        matches!(self, Self::Warning)
    }

    /// Returns `true` if the severity is `Info`.
    pub const fn is_info(&self) -> bool {
        matches!(self, Self::Info)
    }

    /// Returns the string representation of the severity.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }

    /// Returns the color to format the severity with.
    pub const fn color(&self) -> Color {
        match self {
            Self::Error => Color::Red,
            Self::Warning => Color::Yellow,
            Self::Info => Color::White,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_source_locations: Vec<SecondarySourceLocation>,
    pub r#type: String,
    pub component: String,
    pub severity: Severity,
    #[serde(default, with = "serde_helpers::display_from_str_opt")]
    pub error_code: Option<u64>,
    pub message: String,
    pub formatted_message: Option<String>,
}

impl Error {
    /// Returns `true` if the error is an error.
    pub const fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    /// Returns `true` if the error is a warning.
    pub const fn is_warning(&self) -> bool {
        self.severity.is_warning()
    }

    /// Returns `true` if the error is an info.
    pub const fn is_info(&self) -> bool {
        self.severity.is_info()
    }
}

/// Tries to mimic Solidity's own error formatting.
///
/// <https://github.com/ethereum/solidity/blob/a297a687261a1c634551b1dac0e36d4573c19afe/liblangutil/SourceReferenceFormatter.cpp#L105>
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !Paint::is_enabled() {
            let msg = self.formatted_message.as_ref().unwrap_or(&self.message);
            self.fmt_severity(f)?;
            f.write_str(": ")?;
            return f.write_str(msg);
        }

        // Error (XXXX): Error Message
        styled(f, self.severity.color().style().bold(), |f| self.fmt_severity(f))?;
        fmt_msg(f, &self.message)?;

        if let Some(msg) = &self.formatted_message {
            let mut lines = msg.lines();

            // skip first line, it should be similar to the error message we wrote above
            lines.next();

            // format the main source location
            fmt_source_location(f, &mut lines)?;

            // format remaining lines as secondary locations
            while let Some(line) = lines.next() {
                f.write_str("\n")?;

                if let Some((note, msg)) = line.split_once(':') {
                    styled(f, Self::secondary_style(), |f| f.write_str(note))?;
                    fmt_msg(f, msg)?;
                } else {
                    f.write_str(line)?;
                }

                fmt_source_location(f, &mut lines)?;
            }
        }

        Ok(())
    }
}

impl Error {
    /// The style of the diagnostic severity.
    pub fn error_style(&self) -> Style {
        self.severity.color().style().bold()
    }

    /// The style of the diagnostic message.
    pub fn message_style() -> Style {
        Color::White.style().bold()
    }

    /// The style of the secondary source location.
    pub fn secondary_style() -> Style {
        Color::Cyan.style().bold()
    }

    /// The style of the source location highlight.
    pub fn highlight_style() -> Style {
        Color::Yellow.style()
    }

    /// The style of the diagnostics.
    pub fn diag_style() -> Style {
        Color::Yellow.style().bold()
    }

    /// The style of the source location frame.
    pub fn frame_style() -> Style {
        Color::Blue.style()
    }

    /// Formats the diagnostic severity:
    ///
    /// ```text
    /// Error (XXXX)
    /// ```
    fn fmt_severity(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.severity.as_str())?;
        if let Some(code) = self.error_code {
            write!(f, " ({code})")?;
        }
        Ok(())
    }
}

/// Calls `fun` in between [`Style::fmt_prefix`] and [`Style::fmt_suffix`].
fn styled<F>(f: &mut fmt::Formatter, style: Style, fun: F) -> fmt::Result
where
    F: FnOnce(&mut fmt::Formatter) -> fmt::Result,
{
    style.fmt_prefix(f)?;
    fun(f)?;
    style.fmt_suffix(f)
}

/// Formats the diagnostic message.
fn fmt_msg(f: &mut fmt::Formatter, msg: &str) -> fmt::Result {
    styled(f, Error::message_style(), |f| {
        f.write_str(": ")?;
        f.write_str(msg.trim_start())
    })
}

/// Colors a Solidity source location:
///
/// ```text
/// --> /home/user/contract.sol:420:69:
///     |
/// 420 |       bad_code()
///     |                ^
/// ```
fn fmt_source_location(f: &mut fmt::Formatter, lines: &mut std::str::Lines) -> fmt::Result {
    // --> source
    if let Some(line) = lines.next() {
        f.write_str("\n")?;

        let arrow = "-->";
        if let Some((left, loc)) = line.split_once(arrow) {
            f.write_str(left)?;
            styled(f, Error::frame_style(), |f| f.write_str(arrow))?;
            f.write_str(loc)?;
        } else {
            f.write_str(line)?;
        }
    }

    // get the next 3 lines
    let Some(line1) = lines.next() else {
        return Ok(());
    };
    let Some(line2) = lines.next() else {
        f.write_str("\n")?;
        f.write_str(line1)?;
        return Ok(());
    };
    let Some(line3) = lines.next() else {
        f.write_str("\n")?;
        f.write_str(line1)?;
        f.write_str("\n")?;
        f.write_str(line2)?;
        return Ok(());
    };

    // line 1, just a frame
    fmt_framed_location(f, line1, None)?;

    // line 2, frame and code; highlight the text based on line 3's carets
    let hl_start = line3.find('^');
    let highlight = hl_start.map(|start| {
        let end = if line3.contains("^ (") {
            // highlight the entire line because of "spans across multiple lines" diagnostic
            line2.len()
        } else if let Some(carets) = line3[start..].find(|c: char| c != '^') {
            // highlight the text that the carets point to
            start + carets
        } else {
            // the carets span the entire third line
            line3.len()
        }
        // bound in case carets span longer than the code they point to
        .min(line2.len());
        (start.min(end)..end, Error::highlight_style())
    });
    fmt_framed_location(f, line2, highlight)?;

    // line 3, frame and maybe highlight, this time till the end unconditionally
    let highlight = hl_start.map(|i| (i..line3.len(), Error::diag_style()));
    fmt_framed_location(f, line3, highlight)
}

/// Colors a single Solidity framed source location line. Part of [`fmt_source_location`].
fn fmt_framed_location(
    f: &mut fmt::Formatter,
    line: &str,
    highlight: Option<(Range<usize>, Style)>,
) -> fmt::Result {
    f.write_str("\n")?;

    if let Some((space_or_line_number, rest)) = line.split_once('|') {
        // if the potential frame is not just whitespace or numbers, don't color it
        if !space_or_line_number.chars().all(|c| c.is_whitespace() || c.is_numeric()) {
            return f.write_str(line);
        }

        styled(f, Error::frame_style(), |f| {
            f.write_str(space_or_line_number)?;
            f.write_str("|")
        })?;

        if let Some((range, style)) = highlight {
            let Range { start, end } = range;
            // Skip highlighting if the range is not valid unicode.
            if !line.is_char_boundary(start) || !line.is_char_boundary(end) {
                f.write_str(rest)
            } else {
                let rest_start = line.len() - rest.len();
                f.write_str(&line[rest_start..start])?;
                styled(f, style, |f| f.write_str(&line[range]))?;
                f.write_str(&line[end..])
            }
        } else {
            f.write_str(rest)
        }
    } else {
        f.write_str(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_unicode() {
        let e = Error {
            source_location: Some(SourceLocation { file: "test/Counter.t.sol".into(), start: 418, end: 462 }),
            secondary_source_locations: vec![],
            r#type: "ParserError".into(),
            component: "general".into(),
            severity: Severity::Error,
            error_code: Some(8936),
            message: "Invalid character in string. If you are trying to use Unicode characters, use a unicode\"...\" string literal.".into(),
            formatted_message: Some("ParserError: Invalid character in string. If you are trying to use Unicode characters, use a unicode\"...\" string literal.\n  --> test/Counter.t.sol:17:21:\n   |\n17 |         console.log(\"1. ownership set correctly as governance: ✓\");\n   |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n\n".into()),
        };
        let s = e.to_string();
        eprintln!("{s}");
        assert!(!s.is_empty());
    }
}
