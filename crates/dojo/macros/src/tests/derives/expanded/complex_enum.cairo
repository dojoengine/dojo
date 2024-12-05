impl ComplexEnumIntrospect<> of dojo::meta::introspect::Introspect<ComplexEnum<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Enum(
            array![
                dojo::meta::FieldLayout {
                    selector: 0, layout: dojo::meta::introspect::Introspect::<u32>::layout()
                },
                dojo::meta::FieldLayout {
                    selector: 1, layout: dojo::meta::introspect::Introspect::<Option<u64>>::layout()
                },
                dojo::meta::FieldLayout {
                    selector: 2,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<u8>::layout(),
                            dojo::meta::introspect::Introspect::<u16>::layout(),
                            dojo::meta::introspect::Introspect::<u32>::layout()
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
                name: 'ComplexEnum',
                attrs: array![].span(),
                children: array![
                    ('VARIANT1', dojo::meta::introspect::Introspect::<u32>::ty()),
                    ('VARIANT2', dojo::meta::introspect::Introspect::<Option<u64>>::ty()),
                    (
                        'VARIANT3',
                        dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Introspect::<u8>::ty(),
                                dojo::meta::introspect::Introspect::<u16>::ty(),
                                dojo::meta::introspect::Introspect::<u32>::ty()
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
