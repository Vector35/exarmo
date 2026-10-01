//! Reading a corpus, running a word through a decoder, and judging the two
//! against each other.

use std::panic;

use crate::normalize;

/// One instruction set's corpus, as a test reads it.
///
/// The implementor is the set, a unit type where a crate holds one corpus
/// and one value per set where it holds two.
pub trait Corpus: Copy + std::panic::RefUnwindSafe {
    /// The corpus, relative to the crate's manifest directory, as a file or
    /// a directory of them.
    fn path(self) -> &'static str;

    /// The word a line's hex names.
    ///
    /// Not always the number the digits spell. A T32 halfword is four digits
    /// and sits in the high half of the word, which is how the decoder takes
    /// it.
    fn word_of(self, hex: &str) -> Option<u32> {
        word_of(hex)
    }

    /// Decode and render one word, or say why there is no rendering.
    ///
    /// Called inside a guard against a panic, so it need not catch one.
    fn render(self, word: u32) -> Outcome;

    /// Decode and render one word inside an IT block, as a tagged case
    /// names it. Only a corpus that answers [`Corpus::condition`] reads a
    /// tag, and [`Corpus::cases`] refuses one anywhere else, so a corpus
    /// that does not need not write this.
    fn render_in(self, _word: u32, tag: &ItTag) -> Outcome {
        unreachable!(
            "{} carries no IT state, so no case is tagged {tag}",
            self.path()
        )
    }

    /// The four bits of the condition a tag names, where this corpus's cases
    /// may be tagged with one. None by default, which makes a tag a reader
    /// error.
    fn condition(self, _name: &str) -> Option<u8> {
        None
    }

    /// How this set's text is rewritten before two spellings are compared.
    ///
    /// Nothing by default. AArch64's corpus is held to what the XML's value
    /// tables name, so it needs none. AArch32's is written by llvm-mc, which
    /// spells two conditions differently.
    fn rewrite(self) -> normalize::Rewrite {
        |text| text.to_string()
    }

    /// The corpus's text, read from the crate's tests directory.
    fn text(self, manifest_dir: &str) -> String {
        read(&std::path::Path::new(manifest_dir).join(self.path()))
    }

    /// The corpus, read from the crate's tests directory, each case knowing
    /// the file and line it stands on.
    fn cases(self, manifest_dir: &str) -> Vec<Case> {
        let path = std::path::Path::new(manifest_dir).join(self.path());
        let mut cases = Vec::new();
        for file in files(&path) {
            let name = file.file_name().map_or_else(
                || file.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is not readable: {e}", file.display()));
            let read = self
                .parse(&text)
                .unwrap_or_else(|e| panic!("{name} is malformed: {e}"));
            cases.extend(read.into_iter().map(|case| Case {
                place: format!("{name}:{}", case.place),
                ..case
            }));
        }
        cases
    }

    /// Read a text of this corpus, one case per line, refusing a tag that
    /// names no condition of it.
    fn parse(self, text: &str) -> Result<Vec<Case>, String> {
        let cases = parse(text, |hex| self.word_of(hex))?;
        for case in &cases {
            if let Some(tag) = &case.tag
                && self.condition(&tag.cond).is_none()
            {
                return Err(format!(
                    "Line {}: {tag} is no IT state a case of {} can be decoded in",
                    case.place,
                    self.path()
                ));
            }
        }
        Ok(cases)
    }

    /// Decode and render one word, catching any panic the decoder raises.
    ///
    /// A caller running many words will want to silence the panic hook first.
    fn run(self, word: u32) -> Outcome {
        catch(|| self.render(word))
    }

    /// Decode and render one word in the IT state a tag names, or outside
    /// any block where there is none, catching any panic.
    fn run_in(self, word: u32, tag: Option<&ItTag>) -> Outcome {
        match tag {
            Some(tag) => catch(|| self.render_in(word, tag)),
            None => self.run(word),
        }
    }

    /// The text in the one form the two sides are compared in.
    fn normalize(self, text: &str) -> String {
        normalize::text(text, self.rewrite())
    }

    /// Whether a rendering is what the corpus expects.
    fn judge(self, case: &Case, outcome: &Outcome) -> Verdict {
        let expects = expected_outcome(&case.expected);
        match outcome {
            Outcome::Rendered { text, .. } => {
                match expects.is_none() && self.normalize(text) == self.normalize(&case.expected) {
                    true => Verdict::Pass,
                    false => Verdict::Fail,
                }
            }
            // A word that is no instruction passes where the case names what
            // it is instead, and disagrees where the case names something
            // else. A panic is neither, and nor is a rendering that failed.
            Outcome::Undecodable(outcome) if expects.is_some() => match expects == Some(*outcome) {
                true => Verdict::Pass,
                false => Verdict::Fail,
            },
            Outcome::Unrendered { .. } | Outcome::Undecodable(_) | Outcome::Panicked(_) => {
                Verdict::Unimplemented
            }
        }
    }
}

/// A corpus's files, the file itself or every `.txt` file of the directory
/// in the order of their names.
fn files(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    if !path.is_dir() {
        return vec![path.to_path_buf()];
    }
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(path)
        .unwrap_or_else(|e| panic!("{} is not readable: {e}", path.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|file| file.extension().is_some_and(|e| e == "txt"))
        .collect();
    files.sort();
    files
}

/// A corpus's text, its files one after another.
pub fn read(path: &std::path::Path) -> String {
    files(path)
        .iter()
        .map(|file| {
            std::fs::read_to_string(file)
                .unwrap_or_else(|e| panic!("{} is not readable: {e}", file.display()))
                + "\n"
        })
        .collect()
}

/// The word eight hex digits name.
pub fn word_of(hex: &str) -> Option<u32> {
    match hex.len() {
        8 => u32::from_str_radix(hex, 16).ok(),
        _ => None,
    }
}

/// The word a T32 line names. A halfword of four digits sits in the high
/// half, which is how the decoder takes it, and a wide instruction's eight
/// digits are its two halfwords as written.
pub fn t32_word_of(hex: &str) -> Option<u32> {
    let bits = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        4 => Some(bits << 16),
        8 => Some(bits),
        _ => None,
    }
}

/// Where in an IT block a T32 case is decoded, as the tag between its hex
/// and its text writes it: `[eq]` inside a block under EQ with more of the
/// block to come, and `[eq last]` its last instruction. A case with no tag is
/// decoded outside any block.
///
/// Each tag stands for one state, whatever case carries it, so that every
/// line decodes on its own and the corpus's order means nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItTag {
    /// The condition, as the runtime's `Cond` writes it.
    pub cond: String,
    /// Whether the case is the last instruction of its block.
    pub last: bool,
}

impl ItTag {
    /// The low four bits of ITSTATE the tag decodes under. Its lowest set
    /// bit is the block's end, so `0b1000` is the last instruction and
    /// `0b0100` one with another after it.
    pub fn mask(&self) -> u8 {
        match self.last {
            true => 0b1000,
            false => 0b0100,
        }
    }

    /// The tag a line writes after its hex, `[eq]` or `[eq last]`, without
    /// the brackets.
    fn read(inner: &str) -> Option<Self> {
        let (cond, last) = match inner.split_once(' ') {
            Some((cond, "last")) => (cond, true),
            Some(_) => return None,
            None => (inner, false),
        };
        (!cond.is_empty() && cond.chars().all(|c| c.is_ascii_lowercase())).then(|| ItTag {
            cond: cond.to_string(),
            last,
        })
    }
}

impl std::fmt::Display for ItTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.last {
            true => write!(f, "[{} last]", self.cond),
            false => write!(f, "[{}]", self.cond),
        }
    }
}

/// One line of the corpus.
#[derive(Debug, Clone)]
pub struct Case {
    /// The word, as the decoder takes it.
    pub encoding: u32,
    /// How the line wrote it, four hex digits for a T32 halfword.
    pub hex: String,
    /// The IT state it is decoded in, none being outside any block.
    pub tag: Option<ItTag>,
    /// The disassembly the corpus expects, or what the word is instead of an
    /// instruction, `UNDEFINED` or `UNALLOCATED`.
    expected: String,
    /// Where it stands, as the file and line, or the line of a text parsed
    /// on its own.
    pub place: String,
}

/// Whether a line carries a case. Blank lines and comments do not.
fn is_case_line(line: &str) -> bool {
    !line.is_empty() && !line.starts_with("//") && !line.starts_with('#')
}

/// The tag a case's text begins with, and the text after it.
pub(crate) fn tagged(text: &str) -> Result<(Option<ItTag>, &str), String> {
    let Some(rest) = text.strip_prefix('[') else {
        return Ok((None, text));
    };
    let (inner, rest) = rest
        .split_once(']')
        .ok_or_else(|| format!("unclosed IT state tag '[{rest}'"))?;
    let tag = ItTag::read(inner)
        .ok_or_else(|| format!("'[{inner}]' is not a condition, or one followed by 'last'"))?;
    Ok((Some(tag), rest.trim_start()))
}

/// Read a corpus, one case per line, `<hex> <expected>` or `<hex> [tag]
/// <expected>`, the tag being read as it is written and not held to any
/// instruction set's conditions. [`Corpus::parse`] holds it to them.
///
/// Fails on a line that is neither blank, a comment, nor a case.
pub fn parse(text: &str, word_of: impl Fn(&str) -> Option<u32>) -> Result<Vec<Case>, String> {
    let mut cases = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if !is_case_line(line) {
            continue;
        }
        let number = index + 1;
        let (hex, expected) = line
            .split_once(' ')
            .ok_or_else(|| format!("Line {number}: missing expected disassembly"))?;
        let encoding = word_of(hex).ok_or_else(|| format!("Line {number}: invalid hex '{hex}'"))?;
        let (tag, expected) = tagged(expected.trim()).map_err(|e| format!("Line {number}: {e}"))?;
        if expected.is_empty() {
            return Err(format!("Line {number}: missing expected disassembly"));
        }
        cases.push(Case {
            encoding,
            hex: hex.to_string(),
            tag,
            expected: expected.to_string(),
            place: number.to_string(),
        });
    }
    Ok(cases)
}

/// The words a corpus names, taking the first token of each line and
/// ignoring the rest, so a bare list of words reads the same as the corpus.
pub fn encodings<'a>(
    text: &'a str,
    word_of: impl Fn(&str) -> Option<u32> + 'a,
) -> impl Iterator<Item = u32> + 'a {
    text.lines()
        .map(str::trim)
        .filter(|line| is_case_line(line))
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(word_of)
}

/// The outcome a case expects where it expects the word to be no
/// instruction. That is the architecture's term for what it is instead, as
/// `DecodeError` writes it, such as `UNDEFINED` or `UNALLOCATED`.
pub fn expected_outcome(text: &str) -> Option<exarmo_core::DecodeError> {
    use exarmo_core::DecodeError::{self, *};
    [UNDEF, NOP, Unallocated, Unpredictable, ReservedHint]
        .into_iter()
        .find(|outcome: &DecodeError| {
            // Every variant named, so that one added to the enum is added to
            // the list above before this builds.
            match outcome {
                UNDEF | NOP | Unallocated | Unpredictable | ReservedHint => {}
            }
            outcome.to_string() == text
        })
}

/// How a word came out of the decoder.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// The word decoded and rendered.
    Rendered {
        /// The disassembly, at the harness address.
        text: String,
        /// The `Instruction`, as Debug prints it.
        instruction: String,
    },
    /// The word decoded, and the formatter does not write it.
    Unrendered {
        /// The `Instruction`, as Debug prints it.
        instruction: String,
    },
    /// There is no instruction, and what the architecture says the word is
    /// instead.
    Undecodable(exarmo_core::DecodeError),
    /// The decoder panicked, with the panic's message.
    Panicked(String),
}

impl Outcome {
    /// What a decode comes to, written as tokens at `address`, as the text or
    /// the reason there is none. The text of a word that is CONSTRAINED
    /// UNPREDICTABLE carries [`exarmo_core::decode::UNPREDICTABLE_MARK`], as
    /// its case expects it.
    pub fn of<I: exarmo_core::Decoded>(
        decoded: Result<I, exarmo_core::DecodeError>,
        address: u64,
    ) -> Self {
        match decoded {
            Ok(inst) => {
                let instruction = format!("{inst:?}");
                match inst.marked_text_at(address) {
                    Ok(text) => Outcome::Rendered { text, instruction },
                    Err(_) => Outcome::Unrendered { instruction },
                }
            }
            Err(outcome) => Outcome::Undecodable(outcome),
        }
    }

    /// What the harness reports as the actual result, the rendering or the
    /// reason there is none.
    pub fn text(&self) -> String {
        match self {
            Self::Rendered { text, .. } => text.clone(),
            Self::Unrendered { instruction } => format!("<unrendered: {instruction}>"),
            // As a case expects it.
            Self::Undecodable(outcome) => outcome.to_string(),
            Self::Panicked(message) => format!("<panic: {message}>"),
        }
    }
}

/// Run one word, catching any panic the decoder raises.
pub fn catch(render: impl FnOnce() -> Outcome + panic::UnwindSafe) -> Outcome {
    match panic::catch_unwind(render) {
        Ok(outcome) => outcome,
        Err(payload) => {
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "unknown panic"
            };
            Outcome::Panicked(message.to_string())
        }
    }
}

/// Judge every case of a corpus, and fail listing every one that is not
/// what it expects, each where it stands, with what was expected above what
/// came out and the characters that differ marked beneath.
///
/// A caller running many words will want to silence the panic hook first,
/// and it is put back before the report.
pub fn check(corpus: impl Corpus, manifest_dir: &str) {
    let cases = corpus.cases(manifest_dir);
    assert!(!cases.is_empty(), "no test data in {}", corpus.path());
    let total = cases.len();
    let colour = coloured();
    let mut report = String::new();
    let mut wrong = 0usize;
    for case in &cases {
        let outcome = corpus.run_in(case.encoding, case.tag.as_ref());
        let verdict = corpus.judge(case, &outcome);
        if verdict == Verdict::Pass {
            continue;
        }
        wrong += 1;
        report.push_str(&difference(case, &outcome, verdict, colour));
    }
    // The caller silenced the hook for the decoder's panics, and this one
    // is the report.
    drop(panic::take_hook());
    assert!(
        wrong == 0,
        "{wrong} of {total} cases are not what the corpus expects\n{report}"
    );
}

/// The colours the report marks what differs in, bold blue for the
/// expectation and bold yellow for the rendering.
const EXPECTED: &str = "1;34";
const ACTUAL: &str = "1;33";

/// Whether the report is coloured. It is where stderr is a terminal and
/// `NO_COLOR` is not set, or where `CLICOLOR_FORCE` asks for it regardless.
fn coloured() -> bool {
    use std::io::IsTerminal;
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0");
    set("CLICOLOR_FORCE") || (!set("NO_COLOR") && std::io::stderr().is_terminal())
}

/// Text with the characters from `from` to `to` in a colour. Blue marks what
/// the corpus expects and yellow what came out, rather than red and green,
/// since either may be the one that is wrong.
fn painted(text: &str, (from, to): (usize, usize), colour: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let part = |range: std::ops::Range<usize>| chars[range].iter().collect::<String>();
    format!(
        "{}\x1b[{colour}m{}\x1b[0m{}",
        part(0..from),
        part(from..to),
        part(to..chars.len())
    )
}

/// One case that is not what it expects, as `check` reports it.
fn difference(case: &Case, outcome: &Outcome, verdict: Verdict, colour: bool) -> String {
    let expected = case.expected.replace('\t', " ");
    let actual = outcome.text().replace('\t', " ");
    let ((e_from, e_to), (a_from, a_to)) = differing(&expected, &actual);
    let hex = match &case.tag {
        Some(tag) => format!("{} {tag}", case.hex),
        None => case.hex.clone(),
    };
    let mut out = match colour {
        true => format!("\n\x1b[1m{}\x1b[0m  {hex}\n", case.place),
        false => format!("\n{}  {hex}\n", case.place),
    };
    match colour {
        true => {
            out.push_str(&format!(
                "  \x1b[34mexpected\x1b[0m  {}\n",
                painted(&expected, (e_from, e_to), EXPECTED)
            ));
            out.push_str(&format!(
                "  \x1b[33mactual\x1b[0m    {}\n",
                painted(&actual, (a_from, a_to), ACTUAL)
            ));
        }
        false => {
            out.push_str(&format!("  expected  {expected}\n"));
            out.push_str(&format!("  actual    {actual}\n"));
            // Only a rendering has characters to compare.
            if verdict == Verdict::Fail && matches!(outcome, Outcome::Rendered { .. }) {
                out.push_str(&format!(
                    "            {}{}\n",
                    " ".repeat(a_from),
                    "^".repeat(a_to.saturating_sub(a_from).max(1))
                ));
            }
        }
    }
    if let Outcome::Rendered { instruction, .. } | Outcome::Unrendered { instruction } = outcome {
        out.push_str(&format!("  decoded   {instruction}\n"));
    }
    out
}

/// Where `actual` departs from `expected`, in characters, for each. The span
/// runs from the first character that differs up to the last, the text
/// either side being the same.
fn differing(expected: &str, actual: &str) -> ((usize, usize), (usize, usize)) {
    let (e, a): (Vec<char>, Vec<char>) = (expected.chars().collect(), actual.chars().collect());
    let before = e.iter().zip(&a).take_while(|(x, y)| x == y).count();
    let after = e
        .iter()
        .rev()
        .zip(a.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
        .min(e.len().min(a.len()) - before);
    ((before, e.len() - after), (before, a.len() - after))
}

/// The verdict on a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The rendering matches the expectation.
    Pass,
    /// The rendering differs from the expectation.
    Fail,
    /// There is no rendering to compare.
    Unimplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corpus of plain words with no rewriting.
    #[derive(Clone, Copy)]
    struct Plain;

    impl Corpus for Plain {
        fn path(self) -> &'static str {
            "plain.txt"
        }

        fn render(self, _word: u32) -> Outcome {
            Outcome::Undecodable(exarmo_core::DecodeError::Unallocated)
        }
    }

    fn case(expected: &str) -> Case {
        Case {
            place: "1".to_string(),
            encoding: 0,
            hex: "00000000".to_string(),
            tag: None,
            expected: expected.to_string(),
        }
    }

    #[test]
    fn reads_cases_and_skips_comments() {
        let text = "// block\n# also a comment\n\nD2800000 mov x0, #0\n14000040  b 0x100\n";
        let cases = parse(text, word_of).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].encoding, 0xD2800000);
        assert_eq!(cases[0].expected, "mov x0, #0");
        assert_eq!(cases[1].expected, "b 0x100");
        assert_eq!(
            encodings(text, word_of).collect::<Vec<_>>(),
            [0xD2800000, 0x14000040]
        );
    }

    #[test]
    fn a_tag_between_the_hex_and_the_text_is_the_it_state() {
        let text = "1c00 adds r0, r0, #0\n1c00 [eq] addeq r0, r0, #0\n\
                    1c00 [ne last] addne r0, r0, #0\nd000 [eq] UNPREDICTABLE\n";
        let cases = parse(text, t32_word_of).unwrap();
        let tags: Vec<Option<String>> = cases
            .iter()
            .map(|c| c.tag.as_ref().map(ItTag::to_string))
            .collect();
        assert_eq!(
            tags,
            [
                None,
                Some("[eq]".to_string()),
                Some("[ne last]".to_string()),
                Some("[eq]".to_string()),
            ]
        );
        assert_eq!(cases[1].expected, "addeq r0, r0, #0");
        assert_eq!(cases[3].expected, "UNPREDICTABLE");
        assert_eq!(
            cases
                .iter()
                .map(|c| c.tag.as_ref().map(ItTag::mask))
                .collect::<Vec<_>>(),
            [None, Some(0b0100), Some(0b1000), Some(0b0100)]
        );
        for malformed in [
            "1c00 [eq\n",
            "1c00 [eq first] add\n",
            "1c00 [EQ] add\n",
            "1c00 [eq]\n",
        ] {
            assert!(parse(malformed, t32_word_of).is_err(), "{malformed}");
        }
        // A corpus with no conditions of its own refuses any tag
        assert!(Plain.parse("00000000 [eq] add\n").is_err());
        assert!(Plain.parse("00000000 add\n").is_ok());
    }

    #[test]
    fn rejects_a_malformed_line() {
        assert!(parse("D2800000\n", word_of).is_err());
        assert!(parse("zz mov\n", word_of).is_err());
    }

    /// A T32 halfword is four digits in the high half of the word, and a
    /// wide instruction eight.
    #[test]
    fn a_halfword_sits_in_the_high_half() {
        assert_eq!(t32_word_of("4408"), Some(0x4408_0000));
        assert_eq!(t32_word_of("e92d4800"), Some(0xe92d_4800));
        assert_eq!(t32_word_of("440"), None);
        assert_eq!(word_of("4408"), None);
    }

    /// An expectation of an outcome passes on a word that is no instruction
    /// for the reason it names, fails on one that is none for another reason
    /// or that renders, and a panic is neither.
    #[test]
    fn judges_an_outcome_expectation() {
        let expected = case("UNALLOCATED");
        let failed = Outcome::Undecodable(exarmo_core::DecodeError::Unallocated);
        assert_eq!(Plain.judge(&expected, &failed), Verdict::Pass);
        let undefined = Outcome::Undecodable(exarmo_core::DecodeError::UNDEF);
        assert_eq!(Plain.judge(&expected, &undefined), Verdict::Fail);
        assert_eq!(Plain.judge(&case("UNDEFINED"), &undefined), Verdict::Pass);
        // Written as `DecodeError` writes it, or it is read as disassembly.
        assert_eq!(
            Plain.judge(&case("undefined"), &undefined),
            Verdict::Unimplemented
        );
        let rendered = Outcome::Rendered {
            text: "nop".to_string(),
            instruction: String::new(),
        };
        assert_eq!(Plain.judge(&expected, &rendered), Verdict::Fail);
        let panicked = Outcome::Panicked("boom".to_string());
        assert_eq!(Plain.judge(&expected, &panicked), Verdict::Unimplemented);
    }

    /// What differs is from the first character that does to the last,
    /// in each of the two, with what they share either side left out.
    #[test]
    fn a_difference_is_the_span_between_what_the_two_share() {
        assert_eq!(differing("add x8, w24", "add w8, w24"), ((4, 5), (4, 5)));
        assert_eq!(
            differing(
                "autiasppc 0x8000000000000008",
                "autiasppc 0x7ffffffffffc0008"
            ),
            ((12, 24), (12, 24))
        );
        assert_eq!(differing("UNDEFINED", "add w15"), ((0, 9), (0, 7)));
        // A shorter rendering, sharing its end with the expectation.
        assert_eq!(
            differing("ldr x0, [x1, #0x8]", "ldr x0, [x1]"),
            ((11, 17), (11, 11))
        );
    }

    /// An encoding that decodes and is not written is not written yet,
    /// rather than written wrongly.
    #[test]
    fn an_unrendered_encoding_is_unimplemented_rather_than_wrong() {
        let outcome = Outcome::Unrendered {
            instruction: "AndRA1 { .. }".into(),
        };
        assert_eq!(
            Plain.judge(&case("andeq r0, r0, r0"), &outcome),
            Verdict::Unimplemented
        );
    }

    /// A number is compared by its value and text by its letters, whatever
    /// radix and case either side writes them in.
    #[test]
    fn normalizes_radix_and_case() {
        assert_eq!(Plain.normalize("add  x0, x1, #0xe"), "add x0, x1, #14");
        assert_eq!(Plain.normalize("sub x0, x1, #-0x2"), "sub x0, x1, #-2");
        assert_eq!(Plain.normalize("ADD X0, X1, #2"), "add x0, x1, #2");
        assert_eq!(
            Plain.normalize("ldr r0, [r1, #-0x10]"),
            "ldr r0, [r1, #-16]"
        );
        assert_ne!(
            Plain.normalize("mov r0, #0xfe"),
            Plain.normalize("mov r0, #-2")
        );
    }
}
