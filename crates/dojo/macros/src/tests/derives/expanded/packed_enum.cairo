impl PackedEnumIntrospect<> of dojo::meta::introspect::Introspect<PackedEnum<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(3)
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Enum(
            array![
                dojo::meta::FieldLayout {
                    selector: 0,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<u32>::layout(),
                            dojo::meta::introspect::Introspect::<u128>::layout()
                        ]
                            .span()
                    )
                },
                dojo::meta::FieldLayout {
                    selector: 1,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<u32>::layout(),
                            dojo::meta::introspect::Introspect::<u128>::layout()
                        ]
                            .span()
                    )
                },
                dojo::meta::FieldLayout {
                    selector: 2,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<u32>::layout(),
                            dojo::meta::introspect::Introspect::<u128>::layout()
                        ]
                            .span()
                    )
                }
            ]
                .span()
        )
    }

    #[inline(always)]
    fn ty() -> dojo::meta::introspect::Ty {
        dojo::meta::introspect::Ty::Enum(
            dojo::meta::introspect::Enum {
                name: 'PackedEnum',
                attrs: array![].span(),
                children: array![
                    (
                        'VARIANT1',
                        dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Introspect::<u32>::ty(),
                                dojo::meta::introspect::Introspect::<u128>::ty()
                            ]
                                .span()
                        )
                    ),
                    (
                        'VARIANT2',
                        dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Introspect::<u32>::ty(),
                                dojo::meta::introspect::Introspect::<u128>::ty()
                            ]
                                .span()
                        )
                    ),
                    (
                        'VARIANT3',
                        dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Introspect::<u32>::ty(),
                                dojo::meta::introspect::Introspect::<u128>::ty()
                            ]
                                .span()
                        )
                    )
                ]
                    .span()
            }
        )
    }
}
