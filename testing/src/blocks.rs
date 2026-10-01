//! A corpus read as its blocks, and the field values its cases never reach.
//!
//! The generator heads each block with the encoding's diagram and the
//! `Encoding` of every form it is written in (`exarmo_gen::codegen::corpus`
//! writes them), so a test can tell which of an encoding's field values the
//! block's cases hold. Which values count is one rule, the same for every
//! field: each value of a field of up to three bits, since each is
//! likely a row of a table or a choice between forms, and the two ends of a
//! wider one, where a register is ZR or SP and an immediate is at its limit.
//! A value some word of the block's forms holds needs a case of that
//! instruction. One that makes every word fail to decode needs a case
//! expecting what the word is instead, `UNDEFINED` say, so that a decode
//! refusing what it ought to take shows in review rather than passing as a
//! value nothing can reach. A value the architecture gives to another
//! encoding is that block's to hold. A block with no field to vary needs a
//! case all the same.
//!
//! The same holds of what the header says the forms read together: every
//! combination of the values each set of fields comes to, since an operand
//! encoded in `size:Q` or an alias chosen on `Rn` and `imm12` together turns
//! on the pair rather than on either alone. And any two fields encoding a
//! register hold the same register in some case, which is where the
//! architecture chooses an alias or leaves a writeback unpredictable. The
//! block's register fields also all hold different registers in some case
//! that holds every bit the header writes `(0)` or `(1)` as it should. Two
//! of them read or written the wrong way round then show in an instruction
//! that is otherwise plain.
//!
//! Three kinds of immediate mean more than their bits do, and the header says
//! which fields make each. A bitmask immediate is every element size
//! `DecodeBitMasks` tells apart, each with its fewest ones, its most and the
//! reserved all, rotated by nothing, by the most, and by `immr` all set. An
//! A32 modified immediate is its byte at every rotation. An 8-bit
//! floating-point immediate is its sign, each value of the three bits its
//! exponent is made from, and its fraction at both ends.
//!
//! A T32 block's header also says what of the IT state its forms read, and
//! each thing read wants cases decoded inside a block. Where a form takes its
//! condition from the state, every form that some word of the block decodes
//! to inside a block wants a case there under a condition other than AL, so
//! that the condition is pinned in its text, and the block wants some case
//! inside a block even where no word of it decodes there. Where the decode
//! asks whether the word is the last of its block, one word wants a case as
//! the last and one as not, a word where the two differ where there is one.
//! The values of the fields are asked of the cases outside a block alone.

use std::collections::HashSet;

use crate::corpus::ItTag;

/// One box of a diagram: its name where it has one, where it sits, and what
/// the header says of each bit, most significant first: `0` or `1` where the
/// encoding fixes it, `x` where the instruction chooses it, and `z` or `o`
/// where the header writes `(0)` or `(1)`, a bit that should be 0 or 1 and
/// that does not decide the encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: Option<String>,
    pub lo: u8,
    pub bits: String,
}

impl Field {
    /// The positions in the word of the bits the instruction chooses, most
    /// significant first.
    fn varying(&self) -> Vec<u8> {
        let top = self.lo + self.bits.len() as u8 - 1;
        self.bits
            .chars()
            .enumerate()
            .filter(|(_, c)| *c == 'x')
            .map(|(i, _)| top - i as u8)
            .collect()
    }

    /// The bits of the word the header says should be 0 or 1, and what each
    /// should be.
    fn should_be(&self) -> (u32, u32) {
        self.placed('z', 'o')
    }

    /// The bits of the word the header writes as `zero` or `one`, and which
    /// of the two each is.
    fn placed(&self, zero: char, one: char) -> (u32, u32) {
        let top = u32::from(self.lo) + self.bits.len() as u32 - 1;
        (u32::from(self.lo)..=top)
            .rev()
            .zip(self.bits.chars())
            .fold((0, 0), |(mask, value), (at, c)| match c {
                _ if c == zero => (mask | 1 << at, value),
                _ if c == one => (mask | 1 << at, value | 1 << at),
                _ => (mask, value),
            })
    }

    /// How the field is named in a gap, by its name or the bits it covers.
    pub fn label(&self) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => format!("{}:{}", self.lo + self.bits.len() as u8 - 1, self.lo),
        }
    }

    /// The field's value in a word.
    pub fn of(&self, word: u32) -> u32 {
        (word >> self.lo) & ((1u64 << self.bits.len()) - 1) as u32
    }

    /// The word with the field set to a value.
    pub fn set(&self, word: u32, value: u32) -> u32 {
        let mask = (((1u64 << self.bits.len()) - 1) as u32) << self.lo;
        (word & !mask) | ((value << self.lo) & mask)
    }

    /// The values a case ought to hold. That is every one where the
    /// instruction chooses up to three bits, and otherwise those bits all
    /// clear and all set. Each carries the bits the header fixes.
    pub fn targets(&self) -> Vec<u32> {
        let varying = self.varying();
        let fixed = self
            .bits
            .chars()
            .rev()
            .enumerate()
            .filter(|(_, c)| *c == '1')
            .fold(0u32, |v, (i, _)| v | 1 << i);
        let place = |choice: u32| {
            varying.iter().rev().enumerate().fold(fixed, |v, (i, at)| {
                v | (((choice >> i) & 1) << (at - self.lo))
            })
        };
        match varying.len() {
            0 => Vec::new(),
            n @ 1..=3 => (0..1u32 << n).map(place).collect(),
            n => vec![place(0), place(((1u64 << n) - 1) as u32)],
        }
    }

    /// Each value with one of the bits the instruction chooses set and the
    /// rest clear, carrying the bits the header fixes, for a field too wide
    /// for every value to be asked of it. A bit read from the wrong place
    /// shows in the one case that sets it alone, where the two ends read the
    /// same whatever order the bits are taken in.
    pub fn bits_alone(&self) -> Vec<u32> {
        let varying = self.varying();
        if varying.len() <= 3 {
            return Vec::new();
        }
        let fixed = self
            .bits
            .chars()
            .rev()
            .enumerate()
            .filter(|(_, c)| *c == '1')
            .fold(0u32, |v, (i, _)| v | 1 << i);
        varying
            .iter()
            .map(|at| fixed | 1 << (at - self.lo))
            .collect()
    }

    /// A value as the header writes the field's bits.
    pub fn written(&self, value: u32) -> String {
        let width = self.bits.len();
        format!("{value:0width$b}")
    }
}

/// What of the IT state a T32 block's forms read, as its header says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItStateRead {
    /// A form is written under the condition the state gives it.
    pub condition: bool,
    /// The choice between the forms turns on whether the word is in a block.
    pub in_block: bool,
    /// The decode asks whether the word is the last of its block.
    pub last_in_block: bool,
}

/// A case of a block: its word, the IT state it is decoded in, and whether
/// it expects the word to be no instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub word: u32,
    pub tag: Option<ItTag>,
    pub fails: bool,
}

/// The state a gap inside an IT block asks for: under NE, whose low bit
/// differs from AL's, so a condition read without its low bit shows, and
/// the last of the block or not.
pub fn inside(last: bool) -> ItTag {
    ItTag {
        cond: "ne".to_string(),
        last,
    }
}

/// One block of the corpus: the encoding, its diagram, its forms, the sets
/// of fields its forms read together and the fields that encode a register,
/// each field by its place in `fields`, and its cases.
#[derive(Debug, Clone)]
pub struct Block {
    pub encoding: String,
    pub fields: Vec<Field>,
    pub forms: Vec<String>,
    pub together: Vec<Vec<usize>>,
    pub registers: Vec<usize>,
    /// Where a bitmask immediate's parts sit in the word, `N` where it has
    /// one, `immr` and `imms`, each as its top and bottom bit.
    pub bitmask: Option<Vec<(String, u8, u8)>>,
    /// Each floating-point immediate's fields, top first.
    pub floating: Vec<Vec<usize>>,
    /// A modified immediate's fields, top first.
    pub modified: Option<Vec<usize>>,
    /// The fields the choice between the forms turns on.
    pub chosen_on: Vec<usize>,
    pub it_state: ItStateRead,
    pub words: Vec<Held>,
}

/// What begins a header line naming fields read together, as
/// `exarmo_gen::codegen::corpus` writes it.
const TOGETHER: &str = "// read together by ";
/// What begins a header line naming the fields that encode a register.
const REGISTERS: &str = "// register fields: ";
/// What begins a header line naming a bitmask immediate's fields.
const BITMASK: &str = "// bitmask immediate ";
/// What begins a header line naming a floating-point immediate's fields.
const FLOATING: &str = "// floating-point immediate ";
/// What begins a header line naming a modified immediate's field.
const MODIFIED: &str = "// modified immediate ";
/// What begins a header line naming the fields the choice of form turns on.
const CHOSEN: &str = "// form chosen on: ";
/// What begins a header line naming what of the IT state the forms read,
/// and how it names each.
const IT_STATE: &str = "// IT state read: ";
const IT_CONDITION: &str = "condition";
const IT_IN_BLOCK: &str = "in block";
const IT_LAST_IN_BLOCK: &str = "last in block";

/// The parts of a concatenation, `N:immr:imms` or `imm13<12>:imm13<11:6>`,
/// split at the colons that join them rather than those within a slice.
fn parts(concatenated: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (at, c) in concatenated.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ':' if depth == 0 => {
                parts.push(&concatenated[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&concatenated[start..]);
    parts
}

/// Where in the word a field, or a slice of one written `imm13<11:6>` or
/// `imm13<12>`, sits, as its top bit and its bottom.
fn placed(fields: &[Field], part: &str) -> Option<(u8, u8)> {
    let (name, slice) = match part.split_once('<') {
        Some((name, slice)) => (name, Some(slice.strip_suffix('>')?)),
        None => (part, None),
    };
    let field = fields.iter().find(|f| f.name.as_deref() == Some(name))?;
    let top = field.lo + field.bits.len() as u8 - 1;
    match slice {
        None => Some((top, field.lo)),
        Some(slice) => {
            let (hi, lo) = slice.split_once(':').unwrap_or((slice, slice));
            Some((
                field.lo + hi.parse::<u8>().ok()?,
                field.lo + lo.parse::<u8>().ok()?,
            ))
        }
    }
}

impl Block {
    /// The bits the header says should be 0 or 1, and what each should be. A
    /// word holding one otherwise is CONSTRAINED UNPREDICTABLE, and still
    /// decodes as the encoding.
    pub fn should_be(&self) -> (u32, u32) {
        self.fields.iter().fold((0, 0), |(mask, value), field| {
            let (m, v) = field.should_be();
            (mask | m, value | v)
        })
    }

    /// The bits the diagram fixes, as a mask and their values.
    pub fn fixed(&self) -> (u32, u32) {
        self.fields.iter().fold((0, 0), |(mask, value), field| {
            let (m, v) = field.placed('0', '1');
            (mask | m, value | v)
        })
    }
}

/// The diagram a header line gives, `// NAME f=bits|bits|...`, with its
/// encoding's name. The boxes run from the top bit down and fill 32 bits,
/// or 16 for a T32 halfword, which sits in the high half of the word the
/// decoder takes with the low half clear, as a case of four digits reads.
fn header(line: &str) -> Option<(String, Vec<Field>)> {
    let (name, diagram) = line.strip_prefix("// ")?.split_once(' ')?;
    let named = |c: char| c.is_ascii_alphanumeric() || c == '_';
    if name.is_empty() || !name.chars().all(named) {
        return None;
    }
    let mut fields = Vec::new();
    let mut top = 32u32;
    for piece in diagram.split('|') {
        let (field, bits) = match piece.split_once('=') {
            Some((field, bits)) if !field.is_empty() && field.chars().all(named) => {
                (Some(field.to_string()), bits)
            }
            Some(_) => return None,
            None => (None, piece),
        };
        let bits = bits.replace("(0)", "z").replace("(1)", "o");
        if bits.is_empty()
            || !bits
                .chars()
                .all(|c| matches!(c, '0' | '1' | 'x' | 'z' | 'o'))
        {
            return None;
        }
        top = top.checked_sub(bits.len() as u32)?;
        fields.push(Field {
            name: field,
            lo: top as u8,
            bits,
        });
    }
    if top == 16 {
        fields.push(Field {
            name: None,
            lo: 0,
            bits: "0".repeat(16),
        });
        top = 0;
    }
    (top == 0).then(|| (name.to_string(), fields))
}

/// The variant a form line names, `// Variant: TEMPLATE`.
fn form(line: &str) -> Option<&str> {
    let (variant, _) = line.strip_prefix("// ")?.split_once(": ")?;
    (variant.starts_with(|c: char| c.is_ascii_uppercase())
        && variant
            .chars()
            .any(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && variant.chars().all(|c| c.is_ascii_alphanumeric()))
    .then_some(variant)
}

/// Read a corpus's blocks. Lines before the first header, and the words of
/// a file with none, belong to no block and are left out.
pub fn blocks(text: &str, word_of: impl Fn(&str) -> Option<u32>) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut in_header = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some((encoding, fields)) = header(line) {
            blocks.push(Block {
                encoding,
                fields,
                forms: Vec::new(),
                together: Vec::new(),
                registers: Vec::new(),
                bitmask: None,
                floating: Vec::new(),
                modified: None,
                chosen_on: Vec::new(),
                it_state: ItStateRead::default(),
                words: Vec::new(),
            });
            in_header = true;
            continue;
        }
        if let (true, Some(block)) = (in_header, blocks.last_mut()) {
            let named = |list: &str| -> Vec<usize> {
                list.split(", ")
                    .filter_map(|name| {
                        block
                            .fields
                            .iter()
                            .position(|f| f.name.as_deref() == Some(name))
                    })
                    .collect()
            };
            if let Some((_, list)) = line
                .strip_prefix(TOGETHER)
                .and_then(|l| l.rsplit_once(": "))
            {
                let group = named(list);
                block.together.push(group);
                continue;
            }
            // `<imm>: N:immr:imms`, the parts in that order and N optional.
            if let Some((_, list)) = line.strip_prefix(BITMASK).and_then(|l| l.split_once(": ")) {
                let placed: Option<Vec<(u8, u8)>> = parts(list)
                    .into_iter()
                    .map(|p| placed(&block.fields, p))
                    .collect();
                let roles: &[&str] = match placed.as_ref().map(Vec::len) {
                    Some(3) => &["N", "immr", "imms"],
                    _ => &["immr", "imms"],
                };
                block.bitmask = placed.map(|placed| {
                    roles
                        .iter()
                        .zip(placed)
                        .map(|(role, (hi, lo))| (role.to_string(), hi, lo))
                        .collect()
                });
                continue;
            }
            if let Some((_, list)) = line.strip_prefix(FLOATING).and_then(|l| l.split_once(": ")) {
                let fields = named(&parts(list).join(", "));
                block.floating.push(fields);
                continue;
            }
            if let Some((_, list)) = line.strip_prefix(MODIFIED).and_then(|l| l.split_once(": ")) {
                block.modified = Some(named(&parts(list).join(", ")));
                continue;
            }
            if let Some(list) = line.strip_prefix(CHOSEN) {
                block.chosen_on = named(list);
                continue;
            }
            if let Some(list) = line.strip_prefix(IT_STATE) {
                let read: Vec<&str> = list.split(", ").collect();
                block.it_state = ItStateRead {
                    condition: read.contains(&IT_CONDITION),
                    in_block: read.contains(&IT_IN_BLOCK),
                    last_in_block: read.contains(&IT_LAST_IN_BLOCK),
                };
                continue;
            }
            if let Some(list) = line.strip_prefix(REGISTERS) {
                block.registers = named(list);
                continue;
            }
            if let Some(variant) = form(line) {
                block.forms.push(variant.to_string());
                continue;
            }
        }
        in_header = false;
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let Some(block) = blocks.last_mut() else {
            continue;
        };
        let (hex, rest) = line.split_once(' ').unwrap_or((line, ""));
        let (Some(word), Ok((tag, expected))) = (word_of(hex), crate::corpus::tagged(rest.trim()))
        else {
            continue;
        };
        block.words.push(Held {
            word,
            tag,
            fails: crate::expected_outcome(expected.trim()).is_some(),
        });
    }
    blocks
}

/// What a word decodes to, as far as a case can say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// An instruction, with the name of its `Encoding`.
    As(String),
    /// No instruction, for a reason a case can expect.
    Fails,
    /// Anything else, a result a case cannot expect.
    Neither,
}

/// A value of a field no case of its block holds, with a word that holds it
/// and what a case of it expects. That is the instruction where the word
/// decodes to one of the block's forms, and otherwise what it is instead,
/// where it is no instruction at all. A value that makes the word another
/// encoding's is that block's to hold. The block's gaps are packed into as
/// few words as they can be, so one case fills every gap naming its word.
///
/// A block with no field to vary names no value. `field` and `value` are
/// empty, and the gap asks for any case of the block.
///
/// A gap inside an IT block carries the tag its case is decoded under.
#[derive(Debug, Clone)]
pub struct Gap {
    pub encoding: String,
    pub field: String,
    pub value: String,
    pub word: u32,
    pub tag: Option<ItTag>,
    pub no_instruction: bool,
}

/// What a gap asking for a case inside an IT block calls the state, in
/// place of a field.
pub const IT_FIELD: &str = "IT state";

/// A case that decodes to none of its block's forms.
#[derive(Debug, Clone)]
pub struct Misfiled {
    pub encoding: String,
    pub word: u32,
    pub tag: Option<ItTag>,
    /// What it decodes to.
    pub decodes_to: Decoded,
}

/// What a corpus's cases leave unreached.
#[derive(Debug, Default)]
pub struct Coverage {
    pub gaps: Vec<Gap>,
    pub misfiled: Vec<Misfiled>,
    /// The blocks with no case, and no word of the diagram found that either
    /// decodes to them or fails to decode.
    pub unreached: Vec<String>,
}

/// What a case of a block ought to hold: fields at values, two register
/// fields holding the same register, or bits of the word at values, with
/// what a gap calls them.
enum Wanted {
    Values(Vec<(usize, u32)>),
    Equal(usize, usize),
    Distinct(Vec<usize>),
    Bits {
        mask: u32,
        value: u32,
        field: String,
        name: String,
    },
}

impl Wanted {
    /// Whether a word holds it.
    fn held(&self, fields: &[Field], word: u32) -> bool {
        match self {
            Wanted::Values(values) => values.iter().all(|&(i, v)| fields[i].of(word) == v),
            Wanted::Equal(a, b) => fields[*a].of(word) == fields[*b].of(word),
            Wanted::Distinct(registers) => {
                let held: HashSet<u32> = registers.iter().map(|&i| fields[i].of(word)).collect();
                held.len() == registers.len()
                    && fields.iter().all(|f| {
                        let (mask, value) = f.should_be();
                        word & mask == value
                    })
            }
            Wanted::Bits { mask, value, .. } => word & mask == *value,
        }
    }

    /// The ways to set a word so that it holds this. That is the values
    /// themselves, or for two registers the one either holds already, or 1.
    fn settings(&self, fields: &[Field], word: u32) -> Vec<Vec<(usize, u32)>> {
        match self {
            Wanted::Values(values) => vec![values.clone()],
            Wanted::Equal(a, b) => {
                let fits =
                    |v: u32| v >> fields[*a].bits.len() == 0 && v >> fields[*b].bits.len() == 0;
                let mut values = vec![fields[*a].of(word), fields[*b].of(word), 1];
                values.dedup();
                values
                    .into_iter()
                    .filter(|&v| fits(v))
                    .map(|v| vec![(*a, v), (*b, v)])
                    .collect()
            }
            // The registers, and each should-be field at its proper value.
            Wanted::Distinct(registers) => {
                let should_be: Vec<(usize, u32)> = fields
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| f.should_be().0 != 0 && !registers.contains(i))
                    .map(|(i, f)| {
                        let (mask, value) = f.should_be();
                        (i, f.of((word & !mask) | value))
                    })
                    .collect();
                distinct_settings(fields, registers)
                    .into_iter()
                    .map(|mut setting| {
                        setting.extend(&should_be);
                        setting
                    })
                    .collect()
            }
            // Each field the bits fall in, at what it holds with them set.
            Wanted::Bits { mask, value, .. } => {
                let set = (word & !mask) | value;
                vec![
                    fields
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| f.set(0, u32::MAX) & mask != 0)
                        .map(|(i, f)| (i, f.of(set)))
                        .collect(),
                ]
            }
        }
    }

    /// How a gap names it, as each field at its value or the two registers.
    fn label(&self, fields: &[Field]) -> (String, String) {
        match self {
            Wanted::Values(values) => (
                values
                    .iter()
                    .map(|&(i, _)| fields[i].label())
                    .collect::<Vec<_>>()
                    .join("+"),
                values
                    .iter()
                    .map(|&(i, v)| fields[i].written(v))
                    .collect::<Vec<_>>()
                    .join("+"),
            ),
            Wanted::Equal(a, b) => (
                format!("{}+{}", fields[*a].label(), fields[*b].label()),
                "same".to_string(),
            ),
            Wanted::Distinct(registers) => (
                registers
                    .iter()
                    .map(|&i| fields[i].label())
                    .collect::<Vec<_>>()
                    .join("+"),
                "distinct".to_string(),
            ),
            Wanted::Bits { field, name, .. } => (field.clone(), name.clone()),
        }
    }
}

/// Words set to fill gaps, each with the fields it was set to fill, so that
/// the next gap is put in a word it can share.
#[derive(Default)]
struct Words(Vec<(u32, Vec<usize>)>);

impl Words {
    /// The slot of a word holding what is wanted, set so that `keeps` allows
    /// it. That is an existing word where one can take it without undoing
    /// what it was set for, and otherwise one of the bases changed to hold
    /// it.
    fn place(
        &mut self,
        wanted: &Wanted,
        fields: &[Field],
        bases: &[u32],
        keeps: impl Fn(u32) -> bool,
    ) -> Option<usize> {
        let apply = |word: u32, settings: &[(usize, u32)]| {
            settings.iter().fold(word, |w, &(i, v)| fields[i].set(w, v))
        };
        for (slot, (word, filled)) in self.0.iter_mut().enumerate() {
            let fits = wanted.settings(fields, *word).into_iter().find(|settings| {
                settings.iter().all(|(i, _)| !filled.contains(i)) && keeps(apply(*word, settings))
            });
            if let Some(settings) = fits {
                *word = apply(*word, &settings);
                filled.extend(settings.iter().map(|&(i, _)| i));
                return Some(slot);
            }
        }
        let (word, settings) = bases.iter().find_map(|&base| {
            wanted
                .settings(fields, base)
                .into_iter()
                .map(|settings| (apply(base, &settings), settings))
                .find(|&(word, _)| keeps(word))
        })?;
        self.0
            .push((word, settings.iter().map(|&(i, _)| i).collect()));
        Some(self.0.len() - 1)
    }
}

/// Everything a block's cases ought to hold: each field's values, each bit
/// of a wide field that encodes no register set alone, every combination of
/// the values of each set of fields read together, and every two register
/// fields holding the same register.
fn wanted(block: &Block) -> Vec<Wanted> {
    let mut wanted: Vec<Wanted> = Vec::new();
    for (index, field) in block.fields.iter().enumerate() {
        let alone = match block.registers.contains(&index) {
            true => Vec::new(),
            false => field.bits_alone(),
        };
        wanted.extend(
            field
                .targets()
                .into_iter()
                .chain(alone)
                .map(|v| Wanted::Values(vec![(index, v)])),
        );
    }
    for group in &block.together {
        let mut combinations: Vec<Vec<(usize, u32)>> = vec![Vec::new()];
        for &index in group {
            let targets = block.fields[index].targets();
            combinations = combinations
                .into_iter()
                .flat_map(|so_far| {
                    targets.iter().map(move |&v| {
                        let mut next = so_far.clone();
                        next.push((index, v));
                        next
                    })
                })
                .collect();
        }
        wanted.extend(combinations.into_iter().map(Wanted::Values));
    }
    for (n, &a) in block.registers.iter().enumerate() {
        for &b in &block.registers[n + 1..] {
            wanted.push(Wanted::Equal(a, b));
        }
    }
    if block.registers.len() > 1 && !distinct_settings(&block.fields, &block.registers).is_empty() {
        wanted.push(Wanted::Distinct(block.registers.clone()));
    }
    if let Some(parts) = &block.bitmask {
        wanted.extend(bitmask_classes(parts, block.fixed()));
    }
    wanted.extend(should_be_classes(block));
    for fields in &block.floating {
        wanted.extend(floating_classes(&block.fields, fields));
    }
    if let Some(fields) = &block.modified {
        wanted.extend(modified_classes(&block.fields, fields));
    }
    wanted
}

/// Ways to set `registers` to registers all different, each a run of
/// consecutive numbers up or down from a small start, kept where every
/// field can hold its number with the bits it writes `(0)` or `(1)` as they
/// should be, which is what the case is held to.
fn distinct_settings(fields: &[Field], registers: &[usize]) -> Vec<Vec<(usize, u32)>> {
    let n = registers.len() as u32;
    let mut settings: Vec<Vec<(usize, u32)>> = Vec::new();
    for start in [1, 2, 0, 3, 5, 8] {
        for descending in [false, true] {
            let setting: Vec<(usize, u32)> = (0..n)
                .map(|k| match descending {
                    false => start + k,
                    true => start + n - 1 - k,
                })
                .zip(registers)
                .map(|(v, &i)| (i, v))
                .collect();
            let fits = setting.iter().all(|&(i, v)| {
                let (mask, value) = fields[i].should_be();
                v >> fields[i].bits.len() == 0 && fields[i].set(0, v) & mask == value
            });
            if fits && !settings.contains(&setting) {
                settings.push(setting);
            }
        }
    }
    settings
}

/// A word holding every bit the header says should be 0 or 1 as it should,
/// and for each box holding such bits a word breaking every one of them,
/// which is CONSTRAINED UNPREDICTABLE and still the encoding.
fn should_be_classes(block: &Block) -> Vec<Wanted> {
    let (mask, value) = block.should_be();
    if mask == 0 {
        return Vec::new();
    }
    std::iter::once(Wanted::Bits {
        mask,
        value,
        field: "should-be".to_string(),
        name: "held".to_string(),
    })
    .chain(block.fields.iter().filter_map(|field| {
        let (mask, value) = field.should_be();
        (mask != 0).then(|| Wanted::Bits {
            mask,
            value: value ^ mask,
            field: field.label(),
            name: "broken".to_string(),
        })
    }))
    .collect()
}

/// The values of a 12-bit modified immediate `A32ExpandImm` reads
/// differently: its byte rotated right by each even amount, twice the top
/// four bits. The byte is `0x81`, whose two ends both move under every
/// rotation, so a rotation taken wrongly shows in either. The immediate is
/// the fields taken top first, split back into them.
fn modified_classes(fields: &[Field], parts: &[usize]) -> Vec<Wanted> {
    let widths: Vec<usize> = parts.iter().map(|&i| fields[i].bits.len()).collect();
    if widths.iter().sum::<usize>() != 12 {
        return Vec::new();
    }
    (0..16u32)
        .map(|rotation| {
            let imm12 = rotation << 8 | 0x81;
            let mut below = 12;
            Wanted::Values(
                parts
                    .iter()
                    .zip(&widths)
                    .map(|(&i, &w)| {
                        below -= w;
                        (i, (imm12 >> below) & ((1 << w) - 1))
                    })
                    .collect(),
            )
        })
        .collect()
}

/// The values of a bitmask immediate `DecodeBitMasks` reads differently.
///
/// The element is `2 << len` bits for `len` the top set bit of `N:NOT(imms)`,
/// so each size from 2 to 64 bits is its own `N` and run of ones at the top
/// of `imms`. Within a size, `imms`' low bits are one less than how many
/// ones the element holds, all of them being reserved, and `immr`'s are how
/// far it is rotated, its bits above the size read as nothing. Each size is
/// asked with the fewest ones, the most allowed and the reserved all, each
/// rotated by nothing, by the most, and by `immr` all set. A class the
/// diagram's fixed bits rule out, the 64-bit elements of a 32-bit
/// instruction's `N` of 0, is left out.
fn bitmask_classes(
    parts: &[(String, u8, u8)],
    (fixed_mask, fixed_value): (u32, u32),
) -> Vec<Wanted> {
    let part = |name: &str| {
        parts
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|&(_, hi, lo)| (hi, lo))
    };
    let (Some(immr), Some(imms)) = (part("immr"), part("imms")) else {
        return Vec::new();
    };
    let n = part("N");
    let at = |(hi, lo): (u8, u8), v: u32| {
        let mask = (((1u64 << (hi - lo + 1)) - 1) as u32) << lo;
        (mask, (v << lo) & mask)
    };
    let mut wanted = Vec::new();
    for len in 1..=6u32 {
        let esize = 1u32 << len;
        let levels = esize - 1;
        let (n_value, top) = match len {
            6 => (1, 0),
            _ => (0, !((2u32 << len) - 1) & 0x3f),
        };
        if n.is_none() && len == 6 {
            continue;
        }
        let mut ones = vec![0, levels - 1, levels];
        ones.dedup();
        let mut rotations = vec![0, levels, 0x3f];
        rotations.dedup();
        for &s in &ones {
            for &r in &rotations {
                let mut settings = vec![at(imms, top | s), at(immr, r)];
                settings.extend(n.map(|n| at(n, n_value)));
                let (mask, value) = settings
                    .iter()
                    .fold((0, 0), |(m, v), &(sm, sv)| (m | sm, v | sv));
                if (value ^ fixed_value) & mask & fixed_mask != 0 {
                    continue;
                }
                wanted.push(Wanted::Bits {
                    mask,
                    value,
                    field: "bitmask".to_string(),
                    name: format!("{esize}-bit elements, {} ones, rotated {r}", s + 1),
                });
            }
        }
    }
    wanted
}

/// The values of an 8-bit floating-point immediate `VFPExpandImm` reads
/// differently: its sign, each of the eight values of the three bits the
/// exponent is made from, and the fraction at both ends. The immediate is
/// the fields taken top first, split back into them.
fn floating_classes(fields: &[Field], parts: &[usize]) -> Vec<Wanted> {
    let widths: Vec<usize> = parts.iter().map(|&i| fields[i].bits.len()).collect();
    if widths.iter().sum::<usize>() != 8 {
        return Vec::new();
    }
    let mut wanted = Vec::new();
    for sign in 0..2u32 {
        for exponent in 0..8u32 {
            for fraction in [0u32, 0xf] {
                let imm8 = sign << 7 | exponent << 4 | fraction;
                let mut below = 8;
                let values = parts
                    .iter()
                    .zip(&widths)
                    .map(|(&i, &w)| {
                        below -= w;
                        (i, (imm8 >> below) & ((1 << w) - 1))
                    })
                    .collect();
                wanted.push(Wanted::Values(values));
            }
        }
    }
    wanted
}

/// Words of a block's diagram to start from where it has no case: the fixed
/// bits with the rest clear, all set, and a few fills between.
fn probes(block: &Block) -> impl Iterator<Item = u32> {
    let (mask, value) = block.fixed();
    let mut fill = 0x9e37_79b9u32;
    [0, u32::MAX]
        .into_iter()
        .chain((0..64).map(move |_| {
            fill ^= fill << 13;
            fill ^= fill >> 17;
            fill ^= fill << 5;
            fill
        }))
        .map(move |bits| value | (bits & !mask))
}

/// Every value each block's cases ought to hold and do not.
///
/// `decode` says what a word decodes to, in the IT state a tag names or
/// outside any block. A value is looked for first as an instruction of the
/// block, and where no word holding it is, as a word that fails to decode. A
/// block with no case of its own is probed from its diagram, and one with no
/// field to vary still needs a case.
pub fn coverage(blocks: &[Block], decode: impl Fn(u32, Option<&ItTag>) -> Decoded) -> Coverage {
    let mut coverage = Coverage::default();
    for block in blocks {
        let forms: HashSet<&str> = block.forms.iter().map(String::as_str).collect();
        let reaches_in = |word: u32, tag: Option<&ItTag>| match decode(word, tag) {
            Decoded::As(e) => forms.contains(e.as_str()),
            _ => false,
        };
        let reaches = |word: u32| reaches_in(word, None);
        let fails = |word: u32| decode(word, None) == Decoded::Fails;
        for held in block.words.iter().filter(|held| !held.fails) {
            if !reaches_in(held.word, held.tag.as_ref()) {
                coverage.misfiled.push(Misfiled {
                    encoding: block.encoding.clone(),
                    word: held.word,
                    tag: held.tag.clone(),
                    decodes_to: decode(held.word, held.tag.as_ref()),
                });
            }
        }
        // The values of the fields are asked of the cases outside a block
        let expecting = |error: bool| -> Vec<u32> {
            block
                .words
                .iter()
                .filter(|held| held.fails == error && held.tag.is_none())
                .map(|held| held.word)
                .collect()
        };
        let (cases, errors) = (expecting(false), expecting(true));
        let gap = |word: u32, field: String, value: String, no_instruction: bool| Gap {
            encoding: block.encoding.clone(),
            field,
            value,
            word,
            tag: None,
            no_instruction,
        };

        let mut bases: Vec<u32> = cases.iter().copied().filter(|&w| reaches(w)).collect();
        bases.extend(
            bases
                .is_empty()
                .then(|| probes(block).find(|&w| reaches(w)))
                .flatten(),
        );
        if bases.is_empty() {
            // No word is the block's, so a word of it that fails is the case.
            if errors.is_empty() {
                match probes(block).find(|&w| fails(w)) {
                    Some(word) => coverage
                        .gaps
                        .push(gap(word, String::new(), String::new(), true)),
                    None => coverage.unreached.push(block.encoding.clone()),
                }
            }
            continue;
        }
        coverage.gaps.extend(inside_a_block(block, &bases, &decode));
        let wanted = wanted(block);
        if wanted.is_empty() && cases.is_empty() {
            coverage
                .gaps
                .push(gap(bases[0], String::new(), String::new(), false));
            continue;
        }

        // Each gap with the slot of the word that holds it, in one set of
        // words or the other.
        let (mut reached, mut failed) = (Words::default(), Words::default());
        let mut held: Vec<(Gap, usize)> = Vec::new();
        let fields = &block.fields;
        for want in &wanted {
            if cases.iter().any(|&w| want.held(fields, w)) {
                continue;
            }
            let (field, value) = want.label(fields);
            let at = |error| gap(0, field.clone(), value.clone(), error);
            if let Some(slot) = reached.place(want, fields, &bases, reaches) {
                held.push((at(false), slot));
            } else if !errors.iter().any(|&w| want.held(fields, w))
                && let Some(slot) = failed.place(want, fields, &bases, fails)
            {
                held.push((at(true), slot));
            }
        }
        for (mut gap, slot) in held {
            let words = if gap.no_instruction {
                &failed
            } else {
                &reached
            };
            gap.word = words.0[slot].0;
            coverage.gaps.push(gap);
        }
    }
    coverage
}

/// The cases a block's header says it wants inside an IT block and it does
/// not hold, each a word decoded under a tag. The words tried are the
/// block's cases, then `bases`.
fn inside_a_block(
    block: &Block,
    bases: &[u32],
    decode: &impl Fn(u32, Option<&ItTag>) -> Decoded,
) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let mut words: Vec<u32> = block.words.iter().map(|held| held.word).collect();
    words.extend(bases);
    let mut seen = HashSet::new();
    words.retain(|&w| seen.insert(w));
    let gap = |word: u32, tag: ItTag, value: String, decoded: &Decoded| Gap {
        encoding: block.encoding.clone(),
        field: IT_FIELD.to_string(),
        value,
        word,
        tag: Some(tag),
        no_instruction: *decoded == Decoded::Fails,
    };
    let form_of = |decoded: &Decoded| match decoded {
        Decoded::As(e) => block.forms.iter().position(|f| f == e),
        _ => None,
    };
    if block.it_state.condition {
        let conditional = |held: &&Held| held.tag.as_ref().is_some_and(|tag| tag.cond != "al");
        let held: Vec<&Held> = block.words.iter().filter(conditional).collect();
        let mut reached = false;
        for (index, form) in block.forms.iter().enumerate() {
            let has = held
                .iter()
                .any(|h| !h.fails && form_of(&decode(h.word, h.tag.as_ref())) == Some(index));
            if has {
                reached = true;
                continue;
            }
            let found = [false, true].into_iter().find_map(|last| {
                let tag = inside(last);
                let word = words
                    .iter()
                    .copied()
                    .find(|&w| form_of(&decode(w, Some(&tag))) == Some(index))?;
                Some((word, tag))
            });
            if let Some((word, tag)) = found {
                reached = true;
                let decoded = decode(word, Some(&tag));
                gaps.push(gap(
                    word,
                    tag,
                    format!("{form} under a condition"),
                    &decoded,
                ));
            }
        }
        // A block no word of which is an instruction inside a block wants
        // a case saying what its words are there instead
        if !reached && held.is_empty() {
            let tag = inside(false);
            let failing = words
                .iter()
                .copied()
                .find(|&w| decode(w, Some(&tag)) == Decoded::Fails);
            if let Some(word) = failing {
                gaps.push(gap(
                    word,
                    tag,
                    "no form under a condition".to_string(),
                    &Decoded::Fails,
                ));
            }
        }
    }
    if block.it_state.last_in_block {
        let tagged = |word: u32, last: bool| {
            block
                .words
                .iter()
                .any(|h| h.word == word && h.tag.as_ref().is_some_and(|t| t.last == last))
        };
        let paired = words.iter().any(|&w| tagged(w, false) && tagged(w, true));
        if !paired {
            let readable = |w: u32, last: bool| decode(w, Some(&inside(last))) != Decoded::Neither;
            let differs =
                |w: u32| decode(w, Some(&inside(false))) != decode(w, Some(&inside(true)));
            let candidates: Vec<u32> = words
                .iter()
                .copied()
                .filter(|&w| readable(w, false) && readable(w, true))
                .collect();
            let started = |w: &u32| tagged(*w, false) || tagged(*w, true);
            let chosen = candidates
                .iter()
                .find(|w| differs(**w) && started(w))
                .or_else(|| candidates.iter().find(|w| differs(**w)))
                .or_else(|| candidates.iter().find(|w| started(w)))
                .or_else(|| candidates.first());
            if let Some(&word) = chosen {
                for last in [false, true] {
                    if !tagged(word, last) {
                        let tag = inside(last);
                        let decoded = decode(word, Some(&tag));
                        let value = match last {
                            true => "last in block",
                            false => "not last in block",
                        };
                        gaps.push(gap(word, tag, value.to_string(), &decoded));
                    }
                }
            }
        }
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "\
// ADD_32 sf=0|op=0|0100010|sh=x|imm12=xxxxxxxxxxxx|Rn=xxxxx|Rd=xxxxx
// Add32: ADD  <Wd>, <Wn>, #<imm>
// MovAdd32: MOV  <Wd>, <Wn>
// register fields: Rn, Rd
// a note
11000000 add w0, w0, #0x0

// EMPTY_1 1111111111111111|imm16=xxxxxxxxxxxxxxxx
// Empty1: EMPTY
";

    #[test]
    fn a_header_reads_into_fields_filling_the_word() {
        let blocks = blocks(TEXT, crate::word_of);
        assert_eq!(blocks.len(), 2);
        let add = &blocks[0];
        assert_eq!(add.encoding, "ADD_32");
        assert_eq!(add.forms, ["Add32", "MovAdd32"]);
        assert_eq!(
            add.words,
            [Held {
                word: 0x1100_0000,
                tag: None,
                fails: false
            }]
        );
        let rd = add.fields.iter().find(|f| f.label() == "Rd").unwrap();
        assert_eq!((rd.lo, rd.bits.as_str()), (0, "xxxxx"));
        assert_eq!(add.fixed(), (0xff80_0000, 0x1100_0000));
        assert!(blocks[1].words.is_empty());
        // A diagram short of 32 bits is not a header.
        assert!(header("// BAD 0|Q=x|size=0").is_none());
        // Unless it is a halfword, which is the high half of the word
        let (_, halfword) = header("// ADD_T1 0001100|Rm=xxx|Rn=xxx|Rd=xxx").unwrap();
        assert_eq!(halfword[3].lo, 16);
        assert_eq!(halfword[4].bits, "0".repeat(16));
    }

    #[test]
    fn a_should_be_bit_is_neither_fixed_nor_chosen_and_is_held_and_broken() {
        // AXFLAG's CRm, which the diagram writes (0)(0)(0)(0).
        let text = "// AXFLAG_M 1101010100000000|0100|CRm=(0)(0)(0)(0)|010|Rt=11111\n\
                    // Axflag: AXFLAG\n\
                    D500405F axflag\n";
        let block = &blocks(text, crate::word_of)[0];
        let crm = block.fields.iter().find(|f| f.label() == "CRm").unwrap();
        assert_eq!(crm.bits, "zzzz");
        assert_eq!(block.should_be(), (0xf00, 0));
        assert_eq!(block.fixed().0 & 0xf00, 0);
        let classes: Vec<(String, String)> = should_be_classes(block)
            .iter()
            .map(|w| w.label(&block.fields))
            .collect();
        assert_eq!(
            classes,
            [
                ("should-be".to_string(), "held".to_string()),
                ("CRm".to_string(), "broken".to_string())
            ]
        );
    }

    #[test]
    fn a_small_field_wants_every_value_and_a_wide_one_its_ends() {
        let field = |bits: &str| Field {
            name: None,
            lo: 0,
            bits: bits.to_string(),
        };
        assert_eq!(field("xx").targets(), [0, 1, 2, 3]);
        assert!(field("xxx").bits_alone().is_empty());
        assert_eq!(field("xxxxx").bits_alone(), [16, 8, 4, 2, 1]);
        assert_eq!(field("1x0xxx").bits_alone(), [48, 36, 34, 33]);
        assert_eq!(field("xxxxx").targets(), [0, 31]);
        assert_eq!(field("1x0x").targets(), [8, 9, 12, 13]);
        assert!(field("01").targets().is_empty());
    }

    #[test]
    fn a_gap_is_a_value_the_forms_hold_and_no_case_does() {
        let text =
            format!("{TEXT}\n// FIXED_1 11010101000000110010001011011111\n// Fixed1: FIXED\n");
        let blocks = blocks(&text, crate::word_of);
        // ADD's sh set is reserved, EMPTY_1 decodes where its top half is
        // set and is another encoding with its immediate all set, and
        // FIXED_1 is the one word.
        let decode = |word: u32, _: Option<&ItTag>| match word {
            w if w & 0xff80_0000 == 0x1100_0000 && w & 1 << 22 != 0 => Decoded::Fails,
            w if w & 0xff80_0000 == 0x1100_0000 => Decoded::As("Add32".to_string()),
            0xffff_ffff => Decoded::As("Other".to_string()),
            w if w >> 16 == 0xffff => Decoded::As("Empty1".to_string()),
            0xd503_22df => Decoded::As("Fixed1".to_string()),
            _ => Decoded::Neither,
        };
        let coverage = coverage(&blocks, decode);
        // A wide field's bits each alone, and the rest.
        let alone = |g: &&Gap| g.value.len() >= 4 && g.value.matches('1').count() == 1;
        let counted = |field: &str| {
            coverage
                .gaps
                .iter()
                .filter(alone)
                .filter(|g| g.field == field)
                .count()
        };
        assert_eq!((counted("imm12"), counted("imm16")), (12, 16));
        // Registers are not asked their bits alone.
        assert_eq!(counted("Rn") + counted("Rd"), 0);
        let gaps: Vec<String> = coverage
            .gaps
            .iter()
            .filter(|g| !alone(g))
            .map(|g| {
                let expects = if g.no_instruction { "none" } else { "case" };
                format!(
                    "{:08X} {} {}={} {expects}",
                    g.word, g.encoding, g.field, g.value
                )
            })
            .collect();
        assert_eq!(
            gaps,
            [
                // One word fills imm12, Rn and Rd at once.
                "11400000 ADD_32 sh=1 none",
                "113FFFFF ADD_32 imm12=111111111111 case",
                "113FFFFF ADD_32 Rn=11111 case",
                "113FFFFF ADD_32 Rd=11111 case",
                // Rn 1 and Rd 2, so that the two the wrong way round show.
                "11200022 ADD_32 Rn+Rd=distinct case",
                "FFFF0000 EMPTY_1 imm16=0000000000000000 case",
                "D50322DF FIXED_1 = case",
            ]
        );
        assert!(coverage.misfiled.is_empty());
        assert!(coverage.unreached.is_empty());
    }

    #[test]
    fn a_group_wants_every_combination_and_registers_want_to_be_equal() {
        let text = "\
// ORR_1 sh=xx|0000000000000000000|Rm=xxxxx|Rd=xxxxx|n=x
// Orr1: ORR  <Rd>, <Rm>
// read together by <Rd>: sh, n
// register fields: Rm, Rd
00000044 orr r2, r1
";
        let blocks = blocks(text, crate::word_of);
        assert_eq!(blocks[0].together, [vec![0, 4]]);
        assert_eq!(blocks[0].registers, [2, 3]);
        // The two registers equal is UNPREDICTABLE, and nothing else fails.
        let decode = |word: u32, _: Option<&ItTag>| {
            let (rm, rd) = ((word >> 6) & 0x1f, (word >> 1) & 0x1f);
            match rm == rd {
                true => Decoded::Fails,
                false => Decoded::As("Orr1".to_string()),
            }
        };
        let coverage = coverage(&blocks, decode);
        let named = |field: &str| -> Vec<&Gap> {
            coverage.gaps.iter().filter(|g| g.field == field).collect()
        };
        // Every combination but the one the case holds.
        let together = named("sh+n");
        assert_eq!(together.len(), 7);
        assert!(together.iter().all(|g| !g.no_instruction));
        let field = |i: usize| &blocks[0].fields[i];
        for gap in &together {
            let (sh, n) = gap.value.split_once('+').unwrap();
            assert_eq!(field(0).written(field(0).of(gap.word)), sh);
            assert_eq!(field(4).written(field(4).of(gap.word)), n);
        }
        // The registers equal, pinned as no instruction.
        let equal = named("Rm+Rd");
        assert_eq!(equal.len(), 1);
        assert!(equal[0].no_instruction);
        assert_eq!(field(2).of(equal[0].word), field(3).of(equal[0].word));
    }

    /// ADD (register) T2 as far as the IT state goes: UNPREDICTABLE where it
    /// writes the PC inside a block and is not the last of it.
    #[test]
    fn a_block_reading_the_it_state_wants_cases_inside_a_block() {
        let text = "\
// ADD_r_T2 010001|op=00|DN=x|Rm=xxxx|Rdn=xxx
// AddRT2: ADD{<c>}{<q>}  {<Rdn>, }<Rdn>, <Rm>
// IT state read: condition, last in block
4400 add r0, r0
4487 add pc, r0
";
        let blocks = blocks(text, crate::t32_word_of);
        assert_eq!(
            blocks[0].it_state,
            ItStateRead {
                condition: true,
                in_block: false,
                last_in_block: true
            }
        );
        let decode = |word: u32, tag: Option<&ItTag>| {
            let d = (word >> 23 & 1) << 3 | (word >> 16 & 7);
            match tag {
                Some(tag) if !tag.last && d == 15 => Decoded::Fails,
                _ => Decoded::As("AddRT2".to_string()),
            }
        };
        let found = coverage(&blocks, decode);
        let gaps: Vec<(u32, String, &str, bool)> = found
            .gaps
            .iter()
            .filter(|g| g.field == IT_FIELD)
            .map(|g| {
                let tag = g.tag.as_ref().map(ItTag::to_string).unwrap_or_default();
                (g.word, tag, g.value.as_str(), g.no_instruction)
            })
            .collect();
        // The last and not the last on the word where the two differ
        assert_eq!(
            gaps,
            [
                (
                    0x4400_0000,
                    "[ne]".to_string(),
                    "AddRT2 under a condition",
                    false
                ),
                (0x4487_0000, "[ne]".to_string(), "not last in block", true),
                (0x4487_0000, "[ne last]".to_string(), "last in block", false),
            ]
        );
        // With them, nothing is wanted inside a block, and a tagged case is
        // held to its form in its own state
        let text = format!(
            "{text}4400 [ne] addne r0, r0\n4487 [ne] UNPREDICTABLE\n\
             4487 [ne last] addne pc, r0\n"
        );
        let coverage = coverage(&super::blocks(&text, crate::t32_word_of), decode);
        assert!(coverage.gaps.iter().all(|g| g.field != IT_FIELD));
        assert!(coverage.misfiled.is_empty());
    }

    #[test]
    fn a_concatenation_is_split_between_its_parts() {
        assert_eq!(parts("N:immr:imms"), ["N", "immr", "imms"]);
        assert_eq!(
            parts("imm13<12>:imm13<11:6>:imm13<5:0>"),
            ["imm13<12>", "imm13<11:6>", "imm13<5:0>"]
        );
        let field = Field {
            name: Some("imm13".to_string()),
            lo: 5,
            bits: "x".repeat(13),
        };
        let fields = [field];
        assert_eq!(placed(&fields, "imm13<12>"), Some((17, 17)));
        assert_eq!(placed(&fields, "imm13<11:6>"), Some((16, 11)));
        assert_eq!(placed(&fields, "imm13"), Some((17, 5)));
    }

    #[test]
    fn a_bitmask_wants_each_element_size_and_its_ends() {
        let parts = vec![
            ("N".to_string(), 22, 22),
            ("immr".to_string(), 21, 16),
            ("imms".to_string(), 15, 10),
        ];
        let classes = bitmask_classes(&parts, (0, 0));
        assert_eq!(classes.len(), 48);
        // 0x5555... is two-bit elements, one one, not rotated.
        let fives = classes.iter().any(|w| {
            matches!(w, Wanted::Bits { mask, value, .. }
                if *mask == 0x7f_fc00 && *value == 0b11_1100 << 10)
        });
        assert!(fives);
        // A 32-bit instruction's N is fixed at 0, which rules out 64-bit elements.
        assert_eq!(bitmask_classes(&parts, (1 << 22, 0)).len(), 42);
    }

    #[test]
    fn a_modified_immediate_wants_its_byte_at_every_rotation() {
        let imm12 = Field {
            name: Some("imm12".to_string()),
            lo: 0,
            bits: "x".repeat(12),
        };
        let classes = modified_classes(std::slice::from_ref(&imm12), &[0]);
        let values: Vec<u32> = classes
            .iter()
            .filter_map(|w| match w {
                Wanted::Values(v) => Some(v[0].1),
                _ => None,
            })
            .collect();
        assert_eq!(values.len(), 16);
        assert_eq!(values[0], 0x081);
        assert_eq!(values[15], 0xf81);
    }

    #[test]
    fn a_floating_point_immediate_wants_its_sign_exponent_and_fraction() {
        let one = |lo: u8| Field {
            name: None,
            lo,
            bits: "x".to_string(),
        };
        let fields: Vec<Field> = (0..8).rev().map(one).collect();
        let classes = floating_classes(&fields, &(0..8).collect::<Vec<_>>());
        assert_eq!(classes.len(), 32);
        // 1101 1111 is the sign set, exponent bits 101 and the fraction all set.
        let wanted: Vec<(usize, u32)> = (0..8).map(|i| (i, (0xdf >> (7 - i)) & 1)).collect();
        assert!(
            classes
                .iter()
                .any(|w| matches!(w, Wanted::Values(v) if *v == wanted))
        );
        // Only an eight-bit immediate is read this way.
        assert!(floating_classes(&fields, &[0, 1, 2]).is_empty());
    }
}
