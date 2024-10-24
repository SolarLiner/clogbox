use clogbox_core::r#enum::{az::CastFrom, enum_iter, Enum, Sequential};
use clogbox_derive::Enum;
use typenum::{Unsigned, U3};

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Enum)]
enum Inner {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Enum)]
enum Outer {
    First,
    Second(Sequential::<U3>),
    Third(Inner),
}

#[test]
fn test_inner_has_correct_size() {
    assert_eq!(4, <Inner as Enum>::Count::USIZE);
}

#[test]
fn test_outer_has_correct_size() {
    assert_eq!(8, <Outer as Enum>::Count::USIZE);
}

#[test]
fn test_inner_cast_from() {
    let actual = [0, 1, 2, 3].map(|i| Inner::cast_from(i));
    let expected = [Inner::A, Inner::B, Inner::C, Inner::D];
    assert_eq!(expected, actual);
}
#[test]
fn test_outer_enum_iter() {
    let expected = enum_iter::<Outer>()
        .map(|e| e.name().to_string())
        .collect::<Vec<_>>();
    insta::assert_csv_snapshot!(expected);
}
