// based on Starkware metaprogramming stuff here:
// https://github.com/starkware-libs/cairo/blob/main/corelib/src/tuple.cairo

use crate::storage::metaprogramming::*;

pub(crate) impl DojoIsTupleTupleSize0 of DojoIsTuple<()>;
pub(crate) impl DojoIsTupleTupleSize1<E0> of DojoIsTuple<(E0,)>;
pub(crate) impl DojoIsTupleTupleSize2<E0, E1> of DojoIsTuple<(E0, E1)>;
pub(crate) impl DojoIsTupleTupleSize3<E0, E1, E2> of DojoIsTuple<(E0, E1, E2)>;
pub(crate) impl DojoIsTupleTupleSize4<E0, E1, E2, E3> of DojoIsTuple<(E0, E1, E2, E3)>;
pub(crate) impl DojoIsTupleTupleSize5<E0, E1, E2, E3, E4> of DojoIsTuple<(E0, E1, E2, E3, E4)>;
pub(crate) impl DojoIsTupleTupleSize6<
    E0, E1, E2, E3, E4, E5,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5)>;
pub(crate) impl DojoIsTupleTupleSize7<
    E0, E1, E2, E3, E4, E5, E6,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6)>;
pub(crate) impl DojoIsTupleTupleSize8<
    E0, E1, E2, E3, E4, E5, E6, E7,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7)>;
pub(crate) impl DojoIsTupleTupleSize9<
    E0, E1, E2, E3, E4, E5, E6, E7, E8,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8)>;
pub(crate) impl DojoIsTupleTupleSize10<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9)>;
pub(crate) impl DojoIsTupleTupleSize11<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10)>;
pub(crate) impl DojoIsTupleTupleSize12<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11)>;
pub(crate) impl DojoIsTupleTupleSize13<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12)>;
pub(crate) impl DojoIsTupleTupleSize14<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13)>;
pub(crate) impl DojoIsTupleTupleSize15<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14)>;
pub(crate) impl DojoIsTupleTupleSize16<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15,
> of DojoIsTuple<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15)>;

pub(crate) impl DojoTupleSplitTupleSize1<E0> of DojoTupleSplit<(E0,)> {
    type Head = E0;
    type Rest = ();
    fn split_head(self: (E0,)) -> (E0, ()) nopanic {
        let (e0,) = self;
        (e0, ())
    }
    fn reconstruct(head: E0, rest: ()) -> (E0,) nopanic {
        (head,)
    }
}

pub(crate) impl DojoTupleSplitTupleSize2<E0, E1> of DojoTupleSplit<(E0, E1)> {
    type Head = E0;
    type Rest = (E1,);
    fn split_head(self: (E0, E1)) -> (E0, (E1,)) nopanic {
        let (e0, e1) = self;
        (e0, (e1,))
    }
    fn reconstruct(head: E0, rest: (E1,)) -> (E0, E1) nopanic {
        let (e1,) = rest;
        (head, e1)
    }
}

pub(crate) impl DojoTupleSplitTupleSize3<E0, E1, E2> of DojoTupleSplit<(E0, E1, E2)> {
    type Head = E0;
    type Rest = (E1, E2);
    fn split_head(self: (E0, E1, E2)) -> (E0, (E1, E2)) nopanic {
        let (e0, e1, e2) = self;
        (e0, (e1, e2))
    }
    fn reconstruct(head: E0, rest: (E1, E2)) -> (E0, E1, E2) nopanic {
        let (e1, e2) = rest;
        (head, e1, e2)
    }
}

pub(crate) impl DojoTupleSplitTupleSize4<E0, E1, E2, E3> of DojoTupleSplit<(E0, E1, E2, E3)> {
    type Head = E0;
    type Rest = (E1, E2, E3);
    fn split_head(self: (E0, E1, E2, E3)) -> (E0, (E1, E2, E3)) nopanic {
        let (e0, e1, e2, e3) = self;
        (e0, (e1, e2, e3))
    }
    fn reconstruct(head: E0, rest: (E1, E2, E3)) -> (E0, E1, E2, E3) nopanic {
        let (e1, e2, e3) = rest;
        (head, e1, e2, e3)
    }
}

pub(crate) impl DojoTupleSplitTupleSize5<
    E0, E1, E2, E3, E4,
> of DojoTupleSplit<(E0, E1, E2, E3, E4)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4);
    fn split_head(self: (E0, E1, E2, E3, E4)) -> (E0, (E1, E2, E3, E4)) nopanic {
        let (e0, e1, e2, e3, e4) = self;
        (e0, (e1, e2, e3, e4))
    }
    fn reconstruct(head: E0, rest: (E1, E2, E3, E4)) -> (E0, E1, E2, E3, E4) nopanic {
        let (e1, e2, e3, e4) = rest;
        (head, e1, e2, e3, e4)
    }
}

pub(crate) impl DojoTupleSplitTupleSize6<
    E0, E1, E2, E3, E4, E5,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5);
    fn split_head(self: (E0, E1, E2, E3, E4, E5)) -> (E0, (E1, E2, E3, E4, E5)) nopanic {
        let (e0, e1, e2, e3, e4, e5) = self;
        (e0, (e1, e2, e3, e4, e5))
    }
    fn reconstruct(head: E0, rest: (E1, E2, E3, E4, E5)) -> (E0, E1, E2, E3, E4, E5) nopanic {
        let (e1, e2, e3, e4, e5) = rest;
        (head, e1, e2, e3, e4, e5)
    }
}

pub(crate) impl DojoTupleSplitTupleSize7<
    E0, E1, E2, E3, E4, E5, E6,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6);
    fn split_head(self: (E0, E1, E2, E3, E4, E5, E6)) -> (E0, (E1, E2, E3, E4, E5, E6)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6) = self;
        (e0, (e1, e2, e3, e4, e5, e6))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6),
    ) -> (E0, E1, E2, E3, E4, E5, E6) nopanic {
        let (e1, e2, e3, e4, e5, e6) = rest;
        (head, e1, e2, e3, e4, e5, e6)
    }
}

pub(crate) impl DojoTupleSplitTupleSize8<
    E0, E1, E2, E3, E4, E5, E6, E7,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7)
    }
}

pub(crate) impl DojoTupleSplitTupleSize9<
    E0, E1, E2, E3, E4, E5, E6, E7, E8,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8)
    }
}

pub(crate) impl DojoTupleSplitTupleSize10<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9)
    }
}

pub(crate) impl DojoTupleSplitTupleSize11<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10)
    }
}

pub(crate) impl DojoTupleSplitTupleSize12<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11)
    }
}

pub(crate) impl DojoTupleSplitTupleSize13<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12)
    }
}

pub(crate) impl DojoTupleSplitTupleSize14<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13)
    }
}

pub(crate) impl DojoTupleSplitTupleSize15<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14)
    }
}

pub(crate) impl DojoTupleSplitTupleSize16<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15,
> of DojoTupleSplit<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15)> {
    type Head = E0;
    type Rest = (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15);
    fn split_head(
        self: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15),
    ) -> (E0, (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15)) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15) = self;
        (e0, (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15))
    }
    fn reconstruct(
        head: E0, rest: (E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15),
    ) -> (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15) nopanic {
        let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15) = rest;
        (head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize0<E> of DojoTupleExtendFront<(), E> {
    type Result = (E,);
    fn extend_front(value: (), element: E) -> (E,) nopanic {
        (element,)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize1<E0, E> of DojoTupleExtendFront<(E0,), E> {
    type Result = (E, E0);
    fn extend_front(value: (E0,), element: E) -> (E, E0) nopanic {
        let (e0,) = value;
        (element, e0)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize2<E0, E1, E> of DojoTupleExtendFront<(E0, E1), E> {
    type Result = (E, E0, E1);
    fn extend_front(value: (E0, E1), element: E) -> (E, E0, E1) nopanic {
        let (e0, e1) = value;
        (element, e0, e1)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize3<
    E0, E1, E2, E,
> of DojoTupleExtendFront<(E0, E1, E2), E> {
    type Result = (E, E0, E1, E2);
    fn extend_front(value: (E0, E1, E2), element: E) -> (E, E0, E1, E2) nopanic {
        let (e0, e1, e2) = value;
        (element, e0, e1, e2)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize4<
    E0, E1, E2, E3, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3), E> {
    type Result = (E, E0, E1, E2, E3);
    fn extend_front(value: (E0, E1, E2, E3), element: E) -> (E, E0, E1, E2, E3) nopanic {
        let (e0, e1, e2, e3) = value;
        (element, e0, e1, e2, e3)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize5<
    E0, E1, E2, E3, E4, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4), E> {
    type Result = (E, E0, E1, E2, E3, E4);
    fn extend_front(value: (E0, E1, E2, E3, E4), element: E) -> (E, E0, E1, E2, E3, E4) nopanic {
        let (e0, e1, e2, e3, e4) = value;
        (element, e0, e1, e2, e3, e4)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize6<
    E0, E1, E2, E3, E4, E5, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5) nopanic {
        let (e0, e1, e2, e3, e4, e5) = value;
        (element, e0, e1, e2, e3, e4, e5)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize7<
    E0, E1, E2, E3, E4, E5, E6, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6) = value;
        (element, e0, e1, e2, e3, e4, e5, e6)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize8<
    E0, E1, E2, E3, E4, E5, E6, E7, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize9<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize10<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize11<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize12<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize13<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize14<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13)
    }
}

pub(crate) impl DojoTupleExtendFrontTupleSize15<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E,
> of DojoTupleExtendFront<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14), E> {
    type Result = (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14);
    fn extend_front(
        value: (E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14), element: E,
    ) -> (E, E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14) = value;
        (element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize0 of DojoTupleSnapForward<()> {
    type SnapForward = ();
    fn snap_forward(self: @()) -> () nopanic {
        ()
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize1<E0> of DojoTupleSnapForward<(E0,)> {
    type SnapForward = (@E0,);
    fn snap_forward(self: @(E0,)) -> (@E0,) nopanic {
        let (e0,) = self;
        (e0,)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize2<E0, E1> of DojoTupleSnapForward<(E0, E1)> {
    type SnapForward = (@E0, @E1);
    fn snap_forward(self: @(E0, E1)) -> (@E0, @E1) nopanic {
        let (e0, e1) = self;
        (e0, e1)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize3<E0, E1, E2> of DojoTupleSnapForward<(E0, E1, E2)> {
    type SnapForward = (@E0, @E1, @E2);
    fn snap_forward(self: @(E0, E1, E2)) -> (@E0, @E1, @E2) nopanic {
        let (e0, e1, e2) = self;
        (e0, e1, e2)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize4<
    E0, E1, E2, E3,
> of DojoTupleSnapForward<(E0, E1, E2, E3)> {
    type SnapForward = (@E0, @E1, @E2, @E3);
    fn snap_forward(self: @(E0, E1, E2, E3)) -> (@E0, @E1, @E2, @E3) nopanic {
        let (e0, e1, e2, e3) = self;
        (e0, e1, e2, e3)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize5<
    E0, E1, E2, E3, E4,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4);
    fn snap_forward(self: @(E0, E1, E2, E3, E4)) -> (@E0, @E1, @E2, @E3, @E4) nopanic {
        let (e0, e1, e2, e3, e4) = self;
        (e0, e1, e2, e3, e4)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize6<
    E0, E1, E2, E3, E4, E5,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5);
    fn snap_forward(self: @(E0, E1, E2, E3, E4, E5)) -> (@E0, @E1, @E2, @E3, @E4, @E5) nopanic {
        let (e0, e1, e2, e3, e4, e5) = self;
        (e0, e1, e2, e3, e4, e5)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize7<
    E0, E1, E2, E3, E4, E5, E6,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6) = self;
        (e0, e1, e2, e3, e4, e5, e6)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize8<
    E0, E1, E2, E3, E4, E5, E6, E7,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize9<
    E0, E1, E2, E3, E4, E5, E6, E7, E8,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize10<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize11<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize12<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize13<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize14<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13)> {
    type SnapForward = (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13);
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize15<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14)> {
    type SnapForward = (
        @E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13, @E14,
    );
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14),
    ) -> (@E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13, @E14) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14)
    }
}

pub(crate) impl DojoTupleSnapForwardTupleSize16<
    E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15,
> of DojoTupleSnapForward<(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15)> {
    type SnapForward = (
        @E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13, @E14, @E15,
    );
    fn snap_forward(
        self: @(E0, E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14, E15),
    ) -> (
        @E0, @E1, @E2, @E3, @E4, @E5, @E6, @E7, @E8, @E9, @E10, @E11, @E12, @E13, @E14, @E15,
    ) nopanic {
        let (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15) = self;
        (e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15)
    }
}

pub(crate) impl DojoSnapRemoveTupleBase of DojoSnapRemove<()> {
    type Result = ();
}

pub(crate) impl DojoSnapRemoveTupleNext<
    T,
    +DojoIsTuple<T>,
    impl TS: DojoTupleSplit<T>,
    impl HeadNoSnap: DojoSnapRemove<TS::Head>,
    impl RestNoSnap: DojoSnapRemove<TS::Rest>,
    impl TEF: DojoTupleExtendFront<RestNoSnap::Result, HeadNoSnap::Result>,
> of DojoSnapRemove<T> {
    type Result = TEF::Result;
}

pub(crate) impl DojoTuplePartialEq<
    T, impl TSF: DojoTupleSnapForward<T>, +DojoTuplePartialEqHelper<TSF::SnapForward>,
> of PartialEq<T> {
    fn eq(lhs: @T, rhs: @T) -> bool {
        DojoTuplePartialEqHelper::eq(TSF::snap_forward(lhs), TSF::snap_forward(rhs))
    }
    fn ne(lhs: @T, rhs: @T) -> bool {
        DojoTuplePartialEqHelper::ne(TSF::snap_forward(lhs), TSF::snap_forward(rhs))
    }
}

// A trait helper for implementing `PartialEq` for tuples.
pub(crate) trait DojoTuplePartialEqHelper<T> {
    fn eq(lhs: T, rhs: T) -> bool;
    fn ne(lhs: T, rhs: T) -> bool;
}

pub(crate) impl DojoTuplePartialEqHelperByPartialEq<
    T, +PartialEq<T>,
> of DojoTuplePartialEqHelper<@T> {
    fn eq(lhs: @T, rhs: @T) -> bool {
        lhs == rhs
    }
    fn ne(lhs: @T, rhs: @T) -> bool {
        lhs != rhs
    }
}

pub(crate) impl DojoTuplePartialEqHelperBaseTuple of DojoTuplePartialEqHelper<()> {
    fn eq(lhs: (), rhs: ()) -> bool {
        true
    }
    fn ne(lhs: (), rhs: ()) -> bool {
        false
    }
}


pub(crate) impl DojoTuplePartialEqHelperNext<
    T,
    impl TS: DojoTupleSplit<T>,
    +DojoTuplePartialEqHelper<TS::Head>,
    +DojoTuplePartialEqHelper<TS::Rest>,
    +Drop<TS::Rest>,
> of DojoTuplePartialEqHelper<T> {
    fn eq(lhs: T, rhs: T) -> bool {
        let (lhs_head, lhs_rest) = TS::split_head(lhs);
        let (rhs_head, rhs_rest) = TS::split_head(rhs);
        DojoTuplePartialEqHelper::<TS::Head>::eq(lhs_head, rhs_head)
            && DojoTuplePartialEqHelper::<TS::Rest>::eq(lhs_rest, rhs_rest)
    }
    fn ne(lhs: T, rhs: T) -> bool {
        let (lhs_head, lhs_rest) = TS::split_head(lhs);
        let (rhs_head, rhs_rest) = TS::split_head(rhs);
        DojoTuplePartialEqHelper::<TS::Head>::ne(lhs_head, rhs_head)
            || DojoTuplePartialEqHelper::<TS::Rest>::ne(lhs_rest, rhs_rest)
    }
}

pub(crate) impl DojoTupleDefaultNext<
    T, impl TS: DojoTupleSplit<T>, +Default<TS::Head>, +Default<TS::Rest>, +Drop<TS::Head>,
> of Default<T> {
    fn default() -> T {
        TS::reconstruct(Default::default(), Default::default())
    }
}

pub(crate) impl DojoTupleNextDestruct<
    T, impl TH: DojoTupleSplit<T>, +Destruct<TH::Head>, +Destruct<TH::Rest>, -Drop<T>,
> of Destruct<T> {
    fn destruct(self: T) nopanic {
        let (_head, _rest) = TH::split_head(self);
    }
}
