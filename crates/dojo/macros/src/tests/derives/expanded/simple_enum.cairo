impl SimpleEnumIntrospect<> of dojo::meta::introspect::Introspect<SimpleEnum<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::Some(1)
    }

    fn layout() -> dojo::meta::Layout {
        dojo::meta::Layout::Enum(
            array![
                dojo::meta::FieldLayout {
                    selector: 0, layout: dojo::meta::Layout::Fixed(array![].span())
                },
                dojo::meta::FieldLayout {
                    selector: 1, layout: dojo::meta::Layout::Fixed(array![].span())
                },
                dojo::meta::FieldLayout {
                    selector: 2, layout: dojo::meta::Layout::Fixed(array![].span())
                }
            ]
                .span()
        )
    }

    #[inline(always)]
    fn ty() -> dojo::meta::introspect::Ty {
        dojo::meta::introspect::Ty::Enum(
            dojo::meta::introspect::Enum {
                name: 'SimpleEnum',
                attrs: array![].span(),
                children: array![
                    ('VARIANT1', dojo::meta::introspect::Ty::Tuple(array![].span())),
                    ('VARIANT2', dojo::meta::introspect::Ty::Tuple(array![].span())),
                    ('VARIANT3', dojo::meta::introspect::Ty::Tuple(array![].span()))
                ]
                    .span()
            }
        )
    }
}
