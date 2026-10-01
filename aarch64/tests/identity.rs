//! An instruction knows which encoding it is, and its mnemonic.

use exarmo_aarch64::{Encoding, Mnemonic};

#[test]
fn every_index_is_an_encoding_with_a_name() {
    for i in 0..Encoding::COUNT {
        let encoding = Encoding::from_index(i).unwrap();
        assert_eq!(encoding as usize, i);
        assert!(!encoding.name().is_empty());
        assert!(!encoding.mnemonic().name().is_empty());
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
