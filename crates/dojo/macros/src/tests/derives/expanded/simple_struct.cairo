impl SimpleStructIntrospect<> of dojo::meta::introspect::Introspect<SimpleStruct<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(3)
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Struct(
            array![
                dojo::meta::FieldLayout {
                    selector: 687013198911006804117413256380548377255056948723479227932116677690621743639,
                    layout: dojo::meta::introspect::Introspect::<u32>::layout()
                },
                dojo::meta::FieldLayout {
                    selector: 573200779692275582020388969134054872186051594998702457223229675092771367647,
                    layout: dojo::meta::Layout::Tuple(
                        array![
                            dojo::meta::introspect::Introspect::<u8>::layout(),
                            dojo::meta::introspect::Introspect::<u16>::layout()
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
                name: 'SimpleStruct',
                attrs: array![].span(),
                children: array![
                    dojo::meta::introspect::Member {
                        name: 'k1',
                        attrs: array!['key'].span(),
                        ty: dojo::meta::introspect::Introspect::<u256>::ty()
                    },
                    dojo::meta::introspect::Member {
                        name: 'v1',
                        attrs: array![].span(),
                        ty: dojo::meta::introspect::Introspect::<u32>::ty()
                    },
                    dojo::meta::introspect::Member {
                        name: 'v2',
                        attrs: array![].span(),
                        ty: dojo::meta::introspect::Ty::Tuple(
                            array![
                                dojo::meta::introspect::Introspect::<u8>::ty(),
                                dojo::meta::introspect::Introspect::<u16>::ty()
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
