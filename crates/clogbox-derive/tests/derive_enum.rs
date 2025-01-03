use clogbox_enum::{enum_iter, Enum, Sequential};
use typenum::U3;

#[derive(Debug, Default, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Enum)]
enum Inner {
    #[default]
    A,
    B,
    C,
    D,
}

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Enum)]
enum Outer {
    First,
    Second(Sequential<U3>),
    Third(Inner),
}

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Enum)]
enum GenericEnumFirst<T> {
    First(T),
    Second,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum MixedEnum<C> {
    First,
    Second(Inner),
    Third(C),
}

#[test]
fn test_inner_has_correct_size() {
    use typenum::Unsigned;
    assert_eq!(4, <Inner as Enum>::Count::USIZE);
}

#[test]
fn test_outer_has_correct_size() {
    use typenum::Unsigned;
    assert_eq!(8, <Outer as Enum>::Count::USIZE);
}

#[test]
fn test_inner_from_usize() {
    let actual = [0, 1, 2, 3].map(Inner::from_usize);
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

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
enum ComplexEnum<T>
where
    T: Default + PartialEq + Clone + std::fmt::Debug,
{
    SimpleVariant,
    ComplexVariant(T),
}

#[test]
fn test_complex_enum_from_usize() {
    let actual_with_inner = [0, 1, 2, 3, 4].map(ComplexEnum::<Inner>::from_usize);
    let expected_with_inner = [
        ComplexEnum::SimpleVariant,
        ComplexEnum::ComplexVariant(Inner::A),
        ComplexEnum::ComplexVariant(Inner::B),
        ComplexEnum::ComplexVariant(Inner::C),
        ComplexEnum::ComplexVariant(Inner::D),
    ];
    assert_eq!(expected_with_inner, actual_with_inner);
}

#[test]
fn test_complex_enum_iter() {
    let expected_names = enum_iter::<GenericEnumFirst<Inner>>()
        .map(|e| e.name().to_string())
        .collect::<Vec<_>>();
    insta::assert_csv_snapshot!(expected_names);
}

#[test]
fn test_generic_enum_from_usize() {
    let actual_with_inner = [0, 1, 2, 3, 4].map(GenericEnumFirst::<Inner>::from_usize);
    let expected_with_inner = [
        GenericEnumFirst::First(Inner::A),
        GenericEnumFirst::First(Inner::B),
        GenericEnumFirst::First(Inner::C),
        GenericEnumFirst::First(Inner::D),
        GenericEnumFirst::Second,
    ];
    assert_eq!(expected_with_inner, actual_with_inner);
}

#[test]
fn test_generic_enum_iter() {
    let expected_names = enum_iter::<GenericEnumFirst<Inner>>()
        .map(|e| e.name().to_string())
        .collect::<Vec<_>>();
    insta::assert_csv_snapshot!(expected_names);
}
