//! Where the choice between an encoding's forms changes, and whether the
//! corpus holds a case on each side of it.
//!
//! An alias is preferred on a relation between fields as often as on a
//! field's value: LSL where `imms + 1 == immr`, MOV where `Rn` is 31 and the
//! shift is none. The values `blocks` asks for land on both sides of such a
//! relation only by chance. So each form is held to a boundary: some case of
//! it, and one step away in the fields the header says the choice turns on,
//! a case of another form of the same block. A step is one field one more or
//! one less, or one of the bits the instruction chooses in it flipped, so
//! the two cases differ where the condition does and nowhere else.
//!
//! A form the cases leave without a boundary is looked for one step from the
//! cases themselves, which finds most. What that does not find is searched
//! for over every value of the fields the choice turns on, which is slow, so
//! it is asked for apart and only when printing gaps. A form found neither
//! way is reported rather than failed: an alias written unconditionally
//! shadows the base form entirely, and nothing short of the search can tell
//! that from a form the corpus merely misses.

use std::collections::HashMap;

use crate::blocks::{Block, Decoded, Gap, IT_FIELD, inside};
use crate::corpus::ItTag;

/// What a gap of this kind is called, in place of a field.
const FORM: &str = "form";

/// The largest number of bits the search enumerates every value of.
const SEARCHED: usize = 24;

/// A word and the IT state it is decoded in, none being outside any block.
type Placed = (u32, Option<ItTag>);

/// Where a step is taken: in a field, by its place in the block's fields, or
/// into or out of an IT block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Along {
    Field(usize),
    State,
}

/// The words one step from a word in the fields the choice turns on, each
/// with where it steps, in the same IT state. Where the choice reads whether
/// the word is in a block, the same word on the other side of that is a
/// step too.
fn steps(block: &Block, (word, tag): &Placed) -> Vec<(Along, Placed)> {
    let mut steps = Vec::new();
    for &index in &block.chosen_on {
        let field = &block.fields[index];
        let width = field.bits.len();
        let chosen: u32 = field
            .bits
            .chars()
            .rev()
            .enumerate()
            .filter(|(_, c)| *c == 'x')
            .fold(0, |v, (i, _)| v | 1 << i);
        let value = field.of(*word);
        let mut values: Vec<u32> = (0..width)
            .filter(|i| chosen & 1 << i != 0)
            .map(|i| value ^ 1 << i)
            .collect();
        values.extend(value.checked_add(1).filter(|v| v >> width == 0));
        values.extend(value.checked_sub(1));
        // One more or less may carry into a bit the diagram fixes
        values.retain(|v| (v ^ value) & !chosen == 0);
        values.sort_unstable();
        values.dedup();
        steps.extend(
            values
                .into_iter()
                .map(|v| (Along::Field(index), (field.set(*word, v), tag.clone()))),
        );
    }
    if block.it_state.in_block {
        let other = match tag {
            Some(_) => None,
            None => Some(inside(false)),
        };
        steps.push((Along::State, (*word, other)));
    }
    steps
}

/// What the cases leave without a boundary.
#[derive(Debug, Default)]
pub struct Boundaries {
    /// A word for each boundary no case holds. The word is of one form and a
    /// case of the other stands one step from it, or, where the search found
    /// both, two gaps name the two words.
    pub gaps: Vec<Gap>,
    /// Each form, with its block, for which no word was found beside a word
    /// of another form.
    pub unreached: Vec<(String, String)>,
}

/// Every form of every block with more than one that no two cases hold on
/// either side of a boundary. `search` enumerates the fields the choice turns
/// on, and whether the word is in an IT block where the choice reads that,
/// for the forms the cases lead to no word of.
pub fn boundaries(
    blocks: &[Block],
    decode: impl Fn(u32, Option<&ItTag>) -> Decoded,
    search: bool,
) -> Boundaries {
    let mut out = Boundaries::default();
    for block in blocks {
        let chosen = !block.chosen_on.is_empty() || block.it_state.in_block;
        if block.forms.len() < 2 || !chosen {
            continue;
        }
        let form_of = |(word, tag): &Placed| match decode(*word, tag.as_ref()) {
            Decoded::As(form) => block.forms.iter().position(|f| *f == form),
            _ => None,
        };
        // Each word the block holds with its form, the gaps' words joining
        // them so that one word can stand beside more than one form. A case
        // is decoded in its own state and held as one inside a block, since
        // the choice reads only whether the word is in one
        let mut held: HashMap<Placed, usize> = block
            .words
            .iter()
            .filter(|held| !held.fails)
            .filter_map(|held| {
                let form = form_of(&(held.word, held.tag.clone()))?;
                Some(((held.word, held.tag.as_ref().map(|_| inside(false))), form))
            })
            .collect();
        let Some(base) = held.keys().map(|(word, _)| *word).min() else {
            continue;
        };
        let mut decoded: HashMap<Placed, Option<usize>> = HashMap::new();
        let mut searched: Option<Vec<Option<usize>>> = None;
        let gap = |(word, tag): Placed, form: usize, other: usize, along: Along| Gap {
            encoding: block.encoding.clone(),
            field: FORM.to_string(),
            value: format!(
                "{} beside {} on {}",
                block.forms[form],
                block.forms[other],
                match along {
                    Along::Field(field) => block.fields[field].label(),
                    Along::State => IT_FIELD.to_string(),
                }
            ),
            word,
            tag,
            no_instruction: false,
        };
        for form in 0..block.forms.len() {
            let beside = |held: &HashMap<Placed, usize>| {
                held.iter().any(|(placed, &f)| {
                    f == form
                        && steps(block, placed)
                            .iter()
                            .any(|(_, n)| held.get(n).is_some_and(|&g| g != form))
                })
            };
            if beside(&held) {
                continue;
            }
            // A word one step from a case, of this form beside another's
            // case or of another form beside this one's
            let mut cases: Vec<(Placed, usize)> =
                held.iter().map(|(p, &f)| (p.clone(), f)).collect();
            cases.sort_unstable();
            let found = cases.iter().find_map(|(placed, f)| {
                steps(block, placed).into_iter().find_map(|(along, next)| {
                    let g = *decoded
                        .entry(next.clone())
                        .or_insert_with(|| form_of(&next));
                    let g = g.filter(|&g| g != *f && (*f == form || g == form))?;
                    Some((next, g, *f, along))
                })
            });
            if let Some((next, f, other, along)) = found {
                held.insert(next.clone(), f);
                out.gaps.push(gap(next, f, other, along));
                continue;
            }
            if !search {
                out.unreached
                    .push((block.encoding.clone(), block.forms[form].clone()));
                continue;
            }
            // Every value of the fields the choice turns on, from a word of
            // the block, each word numbered by those bits alone, and after
            // them whether it is in a block where the choice reads that
            let positions: Vec<u32> = block
                .chosen_on
                .iter()
                .flat_map(|&i| {
                    let field = &block.fields[i];
                    let top = u32::from(field.lo) + field.bits.len() as u32 - 1;
                    field
                        .bits
                        .chars()
                        .enumerate()
                        .filter(|(_, c)| *c == 'x')
                        .map(move |(at, _)| top - at as u32)
                })
                .collect();
            if positions.len() > SEARCHED {
                out.unreached
                    .push((block.encoding.clone(), block.forms[form].clone()));
                continue;
            }
            let states = 1 + u32::from(block.it_state.in_block);
            let placed_at = |n: u32| -> Placed {
                let (bits, state) = (n / states, n % states);
                let word = positions.iter().enumerate().fold(base, |w, (i, &at)| {
                    (w & !(1 << at)) | ((bits >> i) & 1) << at
                });
                (word, (state == 1).then(|| inside(false)))
            };
            let number = |(word, tag): &Placed| {
                let bits = positions
                    .iter()
                    .enumerate()
                    .fold(0u32, |n, (i, &at)| n | ((word >> at) & 1) << i);
                bits * states + u32::from(tag.is_some())
            };
            let space = searched.get_or_insert_with(|| {
                (0..(1u32 << positions.len()) * states)
                    .map(|n| form_of(&placed_at(n)))
                    .collect()
            });
            let found = (0..space.len() as u32)
                .filter(|&n| space[n as usize] == Some(form))
                .find_map(|n| {
                    let placed = placed_at(n);
                    steps(block, &placed).into_iter().find_map(|(along, next)| {
                        let g = space[number(&next) as usize].filter(|&g| g != form)?;
                        Some((placed.clone(), next, g, along))
                    })
                });
            match found {
                Some((placed, next, other, along)) => {
                    held.insert(placed.clone(), form);
                    held.insert(next.clone(), other);
                    out.gaps.push(gap(placed, form, other, along));
                    out.gaps.push(gap(next, other, form, along));
                }
                None => out
                    .unreached
                    .push((block.encoding.clone(), block.forms[form].clone())),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UBFM as far as LSL and LSR go: LSR where `imms` is all set, LSL where
    /// `imms + 1 == immr` and `imms` is not, UBFM otherwise.
    const TEXT: &str = "\
// UBFM_32 0101001100|immr=xxxxxx|imms=xxxxxx|Rn=xxxxx|Rd=xxxxx
// Ubfm: UBFM  <Wd>, <Wn>, #<immr>, #<imms>
// Lsl: LSL  <Wd>, <Wn>, #<shift>
// Lsr: LSR  <Wd>, <Wn>, #<shift>
// form chosen on: immr, imms
";

    fn decode(word: u32, _: Option<&ItTag>) -> Decoded {
        let (immr, imms) = ((word >> 16) & 0x3f, (word >> 10) & 0x3f);
        Decoded::As(
            match (imms, immr) {
                (0x1f, _) => "Lsr",
                (s, r) if s != 0x1f && s + 1 == r => "Lsl",
                _ => "Ubfm",
            }
            .to_string(),
        )
    }

    fn read(cases: &str) -> Vec<Block> {
        crate::blocks::blocks(&format!("{TEXT}{cases}"), crate::word_of)
    }

    #[test]
    fn a_step_is_one_field_one_more_or_less_or_a_bit_flipped() {
        let blocks = read("");
        assert_eq!(blocks[0].chosen_on, [1, 2]);
        // imms 0: its six bits flipped and one more, and immr's the same
        let word = 0x5300_0000 | 1 << 16;
        let stepped: Vec<u32> = steps(&blocks[0], &(word, None))
            .iter()
            .map(|s| s.1.0)
            .collect();
        assert_eq!(stepped.len(), 6 + 7);
        assert!(stepped.contains(&(word | 1 << 10)));
        assert!(stepped.contains(&(0x5300_0000)));
        assert!(stepped.contains(&(0x5300_0000 | 2 << 16)));
    }

    #[test]
    fn a_form_is_held_where_a_case_of_another_stands_one_step_away() {
        // LSL (immr 1, imms 0) and UBFM (immr 1, imms 1) hold each other,
        // and UBFM with imms 30 is one bit from an LSR no case holds
        let lsl = 0x5300_0000 | 1 << 16;
        let ubfm = lsl | 1 << 10;
        let near = 0x5300_0000 | 30 << 10;
        let cases = format!("{lsl:08X} lsl\n{ubfm:08X} ubfm\n{near:08X} ubfm\n");
        let out = boundaries(&read(&cases), decode, false);
        assert!(out.unreached.is_empty(), "{:?}", out.unreached);
        let gaps: Vec<(u32, &str)> = out
            .gaps
            .iter()
            .map(|g| (g.word, g.value.as_str()))
            .collect();
        assert_eq!(gaps, [(near | 1 << 10, "Lsr beside Ubfm on imms")]);
        // And with it a case, nothing is wanted
        let cases = format!("{cases}{:08X} lsr\n", near | 1 << 10);
        let out = boundaries(&read(&cases), decode, false);
        assert!(out.gaps.is_empty() && out.unreached.is_empty());
    }

    /// ADD (immediate) T1 is ADDS outside an IT block and ADD inside one,
    /// so the one word is held on both sides of that.
    #[test]
    fn a_form_chosen_inside_a_block_is_held_beside_the_same_word_outside() {
        let text = "\
// ADD_i_T1 0001110|imm3=xxx|Rn=xxx|Rd=xxx
// AddIT1: ADD<c>{<q>}  <Rd>, <Rn>, #<imm3>
// T1bAddIT1: ADDS{<q>}  <Rd>, <Rn>, #<imm3>
// IT state read: condition, in block
1c00 adds r0, r0, #0
";
        let decode = |_: u32, tag: Option<&ItTag>| {
            Decoded::As(tag.map_or("T1bAddIT1", |_| "AddIT1").to_string())
        };
        let blocks = crate::blocks::blocks(text, crate::t32_word_of);
        let out = boundaries(&blocks, decode, false);
        let gaps: Vec<(u32, Option<String>, &str)> = out
            .gaps
            .iter()
            .map(|g| {
                (
                    g.word,
                    g.tag.as_ref().map(ItTag::to_string),
                    g.value.as_str(),
                )
            })
            .collect();
        assert_eq!(
            gaps,
            [(
                0x1c00_0000,
                Some("[ne]".to_string()),
                "AddIT1 beside T1bAddIT1 on IT state"
            )]
        );
        let text = format!("{text}1c00 [eq last] addeq r0, r0, #0\n");
        let blocks = crate::blocks::blocks(&text, crate::t32_word_of);
        let out = boundaries(&blocks, decode, false);
        assert!(out.gaps.is_empty() && out.unreached.is_empty());
    }

    #[test]
    fn a_form_the_cases_lead_nowhere_near_is_searched_for() {
        // One case, UBFM #2, #5, from which no step is LSL or LSR
        let ubfm = 0x5300_0000 | 2 << 16 | 5 << 10;
        let blocks = read(&format!("{ubfm:08X} ubfm\n"));
        let out = boundaries(&blocks, decode, false);
        assert!(!out.unreached.is_empty());
        let out = boundaries(&blocks, decode, true);
        assert!(out.unreached.is_empty(), "{:?}", out.unreached);
        // Each form's words, and each beside a word of another form
        for gap in &out.gaps {
            assert!(gap.value.contains(" beside "));
            assert_eq!(gap.word & 0xffc0_03ff, 0x5300_0000 & 0xffc0_03ff);
        }
    }
}
