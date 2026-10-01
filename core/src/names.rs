//! Names packed into one string, as the generated tables hold them.
//!
//! A table of thousands of names as `&str`s is a pointer and a length per
//! name, and a relocation for each. The generated tables run the names
//! together into one string and keep where each ends. The generated source
//! lists the names one to a line and the packing is done at compile time, so
//! a name added to a table changes one line of it rather than every offset
//! after it.

/// How long the names are together.
pub const fn packed_len(names: &[&str]) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < names.len() {
        len += names[i].len();
        i += 1;
    }
    len
}

/// The names run together, `LEN` being their [`packed_len`].
pub const fn pack<const LEN: usize>(names: &[&str]) -> [u8; LEN] {
    let mut text = [0; LEN];
    let mut at = 0;
    let mut i = 0;
    while i < names.len() {
        let name = names[i].as_bytes();
        let mut j = 0;
        while j < name.len() {
            text[at] = name[j];
            at += 1;
            j += 1;
        }
        i += 1;
    }
    assert!(at == LEN, "a table's length is not its names' length");
    text
}

/// The offset just past each name of [`pack`]'s text, `N` being how many
/// names there are.
pub const fn ends<const N: usize>(names: &[&str]) -> [u32; N] {
    assert!(names.len() == N, "a table's count is not its names' count");
    let mut ends = [0; N];
    let mut end = 0;
    let mut i = 0;
    while i < N {
        end += names[i].len();
        ends[i] = end as u32;
        i += 1;
    }
    ends
}

/// [`pack`]'s text as the string it is.
pub const fn text(packed: &'static [u8]) -> &'static str {
    match core::str::from_utf8(packed) {
        Ok(text) => text,
        Err(_) => panic!("a table's names are not UTF-8"),
    }
}

/// The name at `index` of a table whose names are packed into `text`, with
/// `ends[i]` the offset just past name `i`.
///
/// It is `const` so that a C face can lay a whole table out as a constant.
pub const fn name_at(text: &'static str, ends: &[u32], index: usize) -> &'static str {
    let start = if index == 0 {
        0
    } else {
        ends[index - 1] as usize
    };
    let (_, rest) = text.as_bytes().split_at(start);
    let (name, _) = rest.split_at(ends[index] as usize - start);
    match core::str::from_utf8(name) {
        Ok(name) => name,
        Err(_) => panic!("a table's ends fall inside a character"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: &[&str] = &["ab", "", "cde"];
    static PACKED: [u8; packed_len(NAMES)] = pack(NAMES);
    static ENDS: [u32; NAMES.len()] = ends(NAMES);

    #[test]
    fn packed_names_read_back_one_by_one() {
        let text = text(&PACKED);
        assert_eq!(text, "abcde");
        let read: Vec<&str> = (0..NAMES.len()).map(|i| name_at(text, &ENDS, i)).collect();
        assert_eq!(read, NAMES);
    }
}
