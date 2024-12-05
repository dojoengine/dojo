impl SimpleStructIntrospect<> of dojo::meta::introspect::Introspect<SimpleStruct<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(3)
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Fixed(array![32, 8, 16].span())
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
