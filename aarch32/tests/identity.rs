//! An instruction knows which encoding it is, and its mnemonic.

use exarmo_aarch32::{Encoding, ItState, Mnemonic, a32, t32};

#[test]
fn an_instruction_names_its_encoding_and_mnemonic() {
    let add = a32::decode_word(0xe0812003).unwrap();
    let encoding = add.encoding();
    assert_eq!(encoding.name(), "AddRA1");
    assert_eq!(add.mnemonic().name(), "add");
    assert_eq!(encoding.mnemonic(), add.mnemonic());
    assert_eq!(Encoding::from_index(encoding as usize), Some(encoding));
    assert_eq!(encoding.length(), 4);

    // The same instruction in T32 is an encoding of its own.
    let add = t32::decode_word(0x4408_0000, ItState::Outside).unwrap();
    assert_eq!(add.encoding().name(), "AddRT2");
    assert_eq!(add.mnemonic().name(), "add");
    assert_eq!(add.encoding().length(), 2);
}

#[test]
fn every_index_is_an_encoding_with_a_name() {
    for i in 0..Encoding::COUNT {
        let encoding = Encoding::from_index(i).unwrap();
        assert_eq!(encoding as usize, i);
        assert!(!encoding.name().is_empty());
        assert!(!encoding.mnemonic().name().is_empty());
        assert!(matches!(encoding.length(), 2 | 4));
    }
    assert_eq!(Encoding::from_index(Encoding::COUNT), None);
}

#[test]
fn the_mnemonics_are_sorted_and_distinct() {
    let names: Vec<&str> = (0..Mnemonic::COUNT)
        .map(|i| Mnemonic::from_index(i).unwrap().name())
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(names, sorted);
    assert_eq!(Mnemonic::from_index(Mnemonic::COUNT), None);
}
