use dojo::meta::layout::{build_legacy_layout, Layout, FieldLayout};
use dojo::meta::Introspect;

#[test]
fn test_build_legacy_layout_option() {
    // Option<T> legacy layout
    let input = Introspect::<Option<u32>>::layout();
    let expected = Layout::Enum(
        [
            dojo::meta::FieldLayout { // Some
            selector: 0, layout: Introspect::<u32>::layout() },
            dojo::meta::FieldLayout { // None
            selector: 1, layout: Layout::Fixed([].span()) },
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Option<u32>: legacy layout failed");
}

#[test]
fn test_build_legacy_layout_simple_enum() {
    // Enum legacy layout - Simple Enum (Enum { A, B, C, D })
    let input = Layout::Enum(
        [
            dojo::meta::FieldLayout { selector: 1, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 2, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 3, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 4, layout: Layout::Fixed([].span()) },
        ]
            .span(),
    );
    let expected = Layout::Enum(
        [
            dojo::meta::FieldLayout { selector: 0, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 1, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 2, layout: Layout::Fixed([].span()) },
            dojo::meta::FieldLayout { selector: 3, layout: Layout::Fixed([].span()) },
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Enum: legacy layout failed");
}

#[test]
fn test_build_legacy_layout_tuple() {
    // Tuple legacy layout - (Option<u32>, u8)
    let input = Layout::Tuple(
        [Introspect::<Option<u32>>::layout(), Layout::Fixed([8].span())].span(),
    );
    let expected = Layout::Tuple(
        [
            Layout::Enum(
                [
                    dojo::meta::FieldLayout { // Some
                        selector: 0, layout: Introspect::<u32>::layout(),
                    },
                    dojo::meta::FieldLayout { // None
                        selector: 1, layout: Layout::Fixed([].span()),
                    },
                ]
                    .span(),
            ),
            Layout::Fixed([8].span()),
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "(Option<u32>, u8): legacy layout failed");
}

#[test]
fn test_build_legacy_layout_array() {
    // Array legacy layout - Array<Option<u32>>
    let input = Introspect::<Array<Option<u32>>>::layout();
    let expected = Layout::Array(
        [
            Layout::Enum(
                [
                    dojo::meta::FieldLayout { // Some
                        selector: 0, layout: Introspect::<u32>::layout(),
                    },
                    dojo::meta::FieldLayout { // None
                        selector: 1, layout: Layout::Fixed([].span()),
                    },
                ]
                    .span(),
            )
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Array<Option<u32>>: legacy layout failed");
}

#[test]
fn test_build_legacy_layout_struct() {
    // Struct legacy layout - Simple struct { x: Option<u32>, y: u32 }
    let input = Layout::Struct(
        [
            FieldLayout { selector: selector!("x"), layout: Introspect::<Option<u32>>::layout() },
            FieldLayout { selector: selector!("y"), layout: Introspect::<u32>::layout() },
        ]
            .span(),
    );
    let expected = Layout::Struct(
        [
            FieldLayout {
                selector: selector!("x"),
                layout: Layout::Enum(
                    [
                        dojo::meta::FieldLayout { // Some
                            selector: 0, layout: Introspect::<u32>::layout(),
                        },
                        dojo::meta::FieldLayout { // None
                            selector: 1, layout: Layout::Fixed([].span()),
                        },
                    ]
                        .span(),
                ),
            },
            FieldLayout { selector: selector!("y"), layout: Introspect::<u32>::layout() },
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Struct: legacy layout failed");
}

#[test]
fn test_build_legacy_layout_nested_enum() {
    // Enum legacy layout - Nested Enum (Enum { A: Array<Option<u32>, B: (Option<u32>, u8) })
    let input = Layout::Enum(
        [
            dojo::meta::FieldLayout {
                selector: 1, layout: Layout::Array([Introspect::<Option<u32>>::layout()].span()),
            },
            dojo::meta::FieldLayout {
                selector: 2,
                layout: Layout::Tuple(
                    [Introspect::<Option<u32>>::layout(), Introspect::<u8>::layout()].span(),
                ),
            },
        ]
            .span(),
    );
    let expected = Layout::Enum(
        [
            dojo::meta::FieldLayout {
                selector: 0,
                layout: Layout::Array(
                    [
                        Layout::Enum(
                            [
                                dojo::meta::FieldLayout { // Some
                                    selector: 0, layout: Introspect::<u32>::layout(),
                                },
                                dojo::meta::FieldLayout { // None
                                    selector: 1, layout: Layout::Fixed([].span()),
                                },
                            ]
                                .span(),
                        )
                    ]
                        .span(),
                ),
            },
            dojo::meta::FieldLayout {
                selector: 1,
                layout: Layout::Tuple(
                    [
                        Layout::Enum(
                            [
                                dojo::meta::FieldLayout { // Some
                                    selector: 0, layout: Introspect::<u32>::layout(),
                                },
                                dojo::meta::FieldLayout { // None
                                    selector: 1, layout: Layout::Fixed([].span()),
                                },
                            ]
                                .span(),
                        ),
                        Introspect::<u8>::layout(),
                    ]
                        .span(),
                ),
            },
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Nested Enum: legacy layout failed");
}

#[test]
fn test_build_legacy_layout_nested_struct() {
    // Struct legacy layout - Nested struct (Struct { x: Array<Option<u32>, b: (Option<u32>, u8) })
    let input = Layout::Struct(
        [
            dojo::meta::FieldLayout {
                selector: selector!("a"),
                layout: Layout::Array([Introspect::<Option<u32>>::layout()].span()),
            },
            dojo::meta::FieldLayout {
                selector: selector!("b"),
                layout: Layout::Tuple(
                    [Introspect::<Option<u32>>::layout(), Introspect::<u8>::layout()].span(),
                ),
            },
        ]
            .span(),
    );
    let expected = Layout::Struct(
        [
            dojo::meta::FieldLayout {
                selector: selector!("a"),
                layout: Layout::Array(
                    [
                        Layout::Enum(
                            [
                                dojo::meta::FieldLayout { // Some
                                    selector: 0, layout: Introspect::<u32>::layout(),
                                },
                                dojo::meta::FieldLayout { // None
                                    selector: 1, layout: Layout::Fixed([].span()),
                                },
                            ]
                                .span(),
                        )
                    ]
                        .span(),
                ),
            },
            dojo::meta::FieldLayout {
                selector: selector!("b"),
                layout: Layout::Tuple(
                    [
                        Layout::Enum(
                            [
                                dojo::meta::FieldLayout { // Some
                                    selector: 0, layout: Introspect::<u32>::layout(),
                                },
                                dojo::meta::FieldLayout { // None
                                    selector: 1, layout: Layout::Fixed([].span()),
                                },
                            ]
                                .span(),
                        ),
                        Introspect::<u8>::layout(),
                    ]
                        .span(),
                ),
            },
        ]
            .span(),
    );

    assert_eq!(build_legacy_layout(input), expected, "Nested Struct: legacy layout failed");
}
