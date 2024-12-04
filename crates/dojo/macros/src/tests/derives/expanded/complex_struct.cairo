impl ComplexStructIntrospect<> of dojo::meta::introspect::Introspect<ComplexStruct<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Struct(
            array![
                dojo::meta::FieldLayout {
                    selector: 687013198911006804117413256380548377255056948723479227932116677690621743639,
                    layout: dojo::meta::introspect::Introspect::<Array<u32>>::layout()
                },
                dojo::meta::FieldLayout {
                    selector: 573200779692275582020388969134054872186051594998702457223229675092771367647,
                    layout: dojo::meta::introspect::Introspect::<Option<u128>>::layout()
                },
                dojo::meta::FieldLayout {
                    selector: 268067745408767739723108330020913373797853558774636706294407751171317330906,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<Array<u8>>::layout(),
                            dojo::meta::introspect::Introspect::<u16>::layout(),
                            dojo::meta::introspect::Introspect::<Option<u64>>::layout()
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
        dojo::meta::introspect::Ty::Struct(
            dojo::meta::introspect::Struct {
                name: 'ComplexStruct',
                attrs: array![].span(),
                children: array![
                    dojo::meta::introspect::Member {
                        name: 'k1',
                        attrs: array!['key'].span(),
                        ty: dojo::meta::introspect::Introspect::<u256>::ty()
                    },
                    dojo::meta::introspect::Member {
                        name: 'k2',
                        attrs: array!['key'].span(),
                        ty: dojo::meta::introspect::Introspect::<u32>::ty()
                    },
                    dojo::meta::introspect::Member {
                        name: 'v1',
                        attrs: array![].span(),
                        ty: dojo::meta::introspect::Ty::Array(
                            array![dojo::meta::introspect::Introspect::<u32>::ty()].span()
                        )
                    },
                    dojo::meta::introspect::Member {
                        name: 'v2',
                        attrs: array![].span(),
                        ty: dojo::meta::introspect::Introspect::<Option<u128>>::ty()
                    },
                    dojo::meta::introspect::Member {
                        name: 'v3',
                        attrs: array![].span(),
                        ty: dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Ty::Array(
                                    array![dojo::meta::introspect::Introspect::<u8>::ty()].span()
                                ),
                                dojo::meta::introspect::Introspect::<u16>::ty(),
                                dojo::meta::introspect::Introspect::<Option<u64>>::ty()
                            ]
                                .span()
                        )
                    }
                ]
                    .span()
            }
        )
    }
}
