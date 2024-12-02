// File: tests/params_integration.rs

use clogbox_core::param::Normalized;
use clogbox_core::param::ParamFlags;
use clogbox_core::param::Params;
use clogbox_derive::{Enum, Params};
use std::default::Default;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
enum TestEnum {
    #[param(
        range = "1.0..=10.0",
        flags = "ParamFlags::HIDDEN",
        default = "Normalized::HALF",
        value_to_string = "v2s_custom"
    )]
    VariantOne,
    #[param(range = "20.0..=30.0")]
    VariantTwo,
}

fn v2s_custom(value: Normalized) -> String {
    format!("Custom: {}", value)
}

#[test]
fn test_metadata() {
    let variant_one = TestEnum::VariantOne;
    let metadata = variant_one.metadata();

    assert_eq!(metadata.range, 1.0..=10.0);
    assert_eq!(metadata.flags, ParamFlags::HIDDEN);
    assert_eq!(metadata.default, Normalized::new(0.5).unwrap());

    let variant_two = TestEnum::VariantTwo;
    let metadata = variant_two.metadata();

    assert_eq!(metadata.range, 20.0..=30.0);
    assert_eq!(metadata.flags, ParamFlags::default());
}

#[test]
fn test_value_to_string() {
    let variant_one = TestEnum::VariantOne;
    let output = variant_one.value_to_string(Normalized::ZERO);
    assert_eq!(output, "Custom: 0");

    let variant_two = TestEnum::VariantTwo;
    let output = variant_two.value_to_string(Normalized::HALF);
    assert_eq!(output, "0.5");
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
enum GenericEnum<T> {
    ValueA,
    GenericVariant(T),
}

#[test]
fn test_generic_enum_deriving_params() {
    let generic_variant = GenericEnum::GenericVariant(TestEnum::VariantOne);
    let metadata = generic_variant.metadata();

    assert_eq!(metadata.range, 1.0..=10.0);
    assert_eq!(metadata.flags, ParamFlags::HIDDEN);
    assert_eq!(metadata.default, Normalized::HALF);

    let output = generic_variant.value_to_string(Normalized::ZERO);
    assert_eq!(output, "Custom: 0");
}