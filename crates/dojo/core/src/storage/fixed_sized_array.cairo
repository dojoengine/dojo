use crate::storage::dojo_store::*;
use crate::storage::metaprogramming::*;
use crate::storage::tuple::*;

pub(crate) impl DojoTupleSplitFixedSizedArraySized1<T> of DojoTupleSplit<[T; 1]> {
    type Head = T;
    type Rest = [T; 0];
    fn split_head(self: [T; 1]) -> (T, [T; 0]) nopanic {
        let [e0] = self;
        (e0, [])
    }
    fn reconstruct(head: T, rest: [T; 0]) -> [T; 1] nopanic {
        let [] = rest;
        [head]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized2<T> of DojoTupleSplit<[T; 2]> {
    type Head = T;
    type Rest = [T; 1];
    fn split_head(self: [T; 2]) -> (T, [T; 1]) nopanic {
        let [e0, e1] = self;
        (e0, [e1])
    }
    fn reconstruct(head: T, rest: [T; 1]) -> [T; 2] nopanic {
        let [e1] = rest;
        [head, e1]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized3<T> of DojoTupleSplit<[T; 3]> {
    type Head = T;
    type Rest = [T; 2];
    fn split_head(self: [T; 3]) -> (T, [T; 2]) nopanic {
        let [e0, e1, e2] = self;
        (e0, [e1, e2])
    }
    fn reconstruct(head: T, rest: [T; 2]) -> [T; 3] nopanic {
        let [e1, e2] = rest;
        [head, e1, e2]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized4<T> of DojoTupleSplit<[T; 4]> {
    type Head = T;
    type Rest = [T; 3];
    fn split_head(self: [T; 4]) -> (T, [T; 3]) nopanic {
        let [e0, e1, e2, e3] = self;
        (e0, [e1, e2, e3])
    }
    fn reconstruct(head: T, rest: [T; 3]) -> [T; 4] nopanic {
        let [e1, e2, e3] = rest;
        [head, e1, e2, e3]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized5<T> of DojoTupleSplit<[T; 5]> {
    type Head = T;
    type Rest = [T; 4];
    fn split_head(self: [T; 5]) -> (T, [T; 4]) nopanic {
        let [e0, e1, e2, e3, e4] = self;
        (e0, [e1, e2, e3, e4])
    }
    fn reconstruct(head: T, rest: [T; 4]) -> [T; 5] nopanic {
        let [e1, e2, e3, e4] = rest;
        [head, e1, e2, e3, e4]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized6<T> of DojoTupleSplit<[T; 6]> {
    type Head = T;
    type Rest = [T; 5];
    fn split_head(self: [T; 6]) -> (T, [T; 5]) nopanic {
        let [e0, e1, e2, e3, e4, e5] = self;
        (e0, [e1, e2, e3, e4, e5])
    }
    fn reconstruct(head: T, rest: [T; 5]) -> [T; 6] nopanic {
        let [e1, e2, e3, e4, e5] = rest;
        [head, e1, e2, e3, e4, e5]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized7<T> of DojoTupleSplit<[T; 7]> {
    type Head = T;
    type Rest = [T; 6];
    fn split_head(self: [T; 7]) -> (T, [T; 6]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6] = self;
        (e0, [e1, e2, e3, e4, e5, e6])
    }
    fn reconstruct(head: T, rest: [T; 6]) -> [T; 7] nopanic {
        let [e1, e2, e3, e4, e5, e6] = rest;
        [head, e1, e2, e3, e4, e5, e6]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized8<T> of DojoTupleSplit<[T; 8]> {
    type Head = T;
    type Rest = [T; 7];
    fn split_head(self: [T; 8]) -> (T, [T; 7]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7])
    }
    fn reconstruct(head: T, rest: [T; 7]) -> [T; 8] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized9<T> of DojoTupleSplit<[T; 9]> {
    type Head = T;
    type Rest = [T; 8];
    fn split_head(self: [T; 9]) -> (T, [T; 8]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8])
    }
    fn reconstruct(head: T, rest: [T; 8]) -> [T; 9] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized10<T> of DojoTupleSplit<[T; 10]> {
    type Head = T;
    type Rest = [T; 9];
    fn split_head(self: [T; 10]) -> (T, [T; 9]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9])
    }
    fn reconstruct(head: T, rest: [T; 9]) -> [T; 10] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized11<T> of DojoTupleSplit<[T; 11]> {
    type Head = T;
    type Rest = [T; 10];
    fn split_head(self: [T; 11]) -> (T, [T; 10]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10])
    }
    fn reconstruct(head: T, rest: [T; 10]) -> [T; 11] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized12<T> of DojoTupleSplit<[T; 12]> {
    type Head = T;
    type Rest = [T; 11];
    fn split_head(self: [T; 12]) -> (T, [T; 11]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11])
    }
    fn reconstruct(head: T, rest: [T; 11]) -> [T; 12] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized13<T> of DojoTupleSplit<[T; 13]> {
    type Head = T;
    type Rest = [T; 12];
    fn split_head(self: [T; 13]) -> (T, [T; 12]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12])
    }
    fn reconstruct(head: T, rest: [T; 12]) -> [T; 13] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized14<T> of DojoTupleSplit<[T; 14]> {
    type Head = T;
    type Rest = [T; 13];
    fn split_head(self: [T; 14]) -> (T, [T; 13]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13])
    }
    fn reconstruct(head: T, rest: [T; 13]) -> [T; 14] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized15<T> of DojoTupleSplit<[T; 15]> {
    type Head = T;
    type Rest = [T; 14];
    fn split_head(self: [T; 15]) -> (T, [T; 14]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14])
    }
    fn reconstruct(head: T, rest: [T; 14]) -> [T; 15] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14]
    }
}

pub(crate) impl DojoTupleSplitFixedSizedArraySized16<T> of DojoTupleSplit<[T; 16]> {
    type Head = T;
    type Rest = [T; 15];
    fn split_head(self: [T; 16]) -> (T, [T; 15]) nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15] = self;
        (e0, [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15])
    }
    fn reconstruct(head: T, rest: [T; 15]) -> [T; 16] nopanic {
        let [e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15] = rest;
        [head, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15]
    }
}


pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize0<T> of DojoTupleExtendFront<[T; 0], T> {
    type Result = [T; 1];
    fn extend_front(value: [T; 0], element: T) -> [T; 1] nopanic {
        let [] = value;
        [element]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize1<T> of DojoTupleExtendFront<[T; 1], T> {
    type Result = [T; 2];
    fn extend_front(value: [T; 1], element: T) -> [T; 2] nopanic {
        let [e0] = value;
        [element, e0]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize2<T> of DojoTupleExtendFront<[T; 2], T> {
    type Result = [T; 3];
    fn extend_front(value: [T; 2], element: T) -> [T; 3] nopanic {
        let [e0, e1] = value;
        [element, e0, e1]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize3<T> of DojoTupleExtendFront<[T; 3], T> {
    type Result = [T; 4];
    fn extend_front(value: [T; 3], element: T) -> [T; 4] nopanic {
        let [e0, e1, e2] = value;
        [element, e0, e1, e2]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize4<T> of DojoTupleExtendFront<[T; 4], T> {
    type Result = [T; 5];
    fn extend_front(value: [T; 4], element: T) -> [T; 5] nopanic {
        let [e0, e1, e2, e3] = value;
        [element, e0, e1, e2, e3]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize5<T> of DojoTupleExtendFront<[T; 5], T> {
    type Result = [T; 6];
    fn extend_front(value: [T; 5], element: T) -> [T; 6] nopanic {
        let [e0, e1, e2, e3, e4] = value;
        [element, e0, e1, e2, e3, e4]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize6<T> of DojoTupleExtendFront<[T; 6], T> {
    type Result = [T; 7];
    fn extend_front(value: [T; 6], element: T) -> [T; 7] nopanic {
        let [e0, e1, e2, e3, e4, e5] = value;
        [element, e0, e1, e2, e3, e4, e5]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize7<T> of DojoTupleExtendFront<[T; 7], T> {
    type Result = [T; 8];
    fn extend_front(value: [T; 7], element: T) -> [T; 8] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6] = value;
        [element, e0, e1, e2, e3, e4, e5, e6]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize8<T> of DojoTupleExtendFront<[T; 8], T> {
    type Result = [T; 9];
    fn extend_front(value: [T; 8], element: T) -> [T; 9] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize9<T> of DojoTupleExtendFront<[T; 9], T> {
    type Result = [T; 10];
    fn extend_front(value: [T; 9], element: T) -> [T; 10] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize10<T> of DojoTupleExtendFront<[T; 10], T> {
    type Result = [T; 11];
    fn extend_front(value: [T; 10], element: T) -> [T; 11] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize11<T> of DojoTupleExtendFront<[T; 11], T> {
    type Result = [T; 12];
    fn extend_front(value: [T; 11], element: T) -> [T; 12] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize12<T> of DojoTupleExtendFront<[T; 12], T> {
    type Result = [T; 13];
    fn extend_front(value: [T; 12], element: T) -> [T; 13] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize13<T> of DojoTupleExtendFront<[T; 13], T> {
    type Result = [T; 14];
    fn extend_front(value: [T; 13], element: T) -> [T; 14] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize14<T> of DojoTupleExtendFront<[T; 14], T> {
    type Result = [T; 15];
    fn extend_front(value: [T; 14], element: T) -> [T; 15] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13]
    }
}

pub(crate) impl DojoTupleExtendFrontFixedSizedArraySize15<T> of DojoTupleExtendFront<[T; 15], T> {
    type Result = [T; 16];
    fn extend_front(value: [T; 15], element: T) -> [T; 16] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14] = value;
        [element, e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14]
    }
}


pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized0<T> of DojoTupleSnapForward<[T; 0]> {
    type SnapForward = [@T; 0];
    fn snap_forward(self: @[T; 0]) -> [@T; 0] nopanic {
        []
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized1<T> of DojoTupleSnapForward<[T; 1]> {
    type SnapForward = [@T; 1];
    fn snap_forward(self: @[T; 1]) -> [@T; 1] nopanic {
        let [e0] = self;
        [e0]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized2<T> of DojoTupleSnapForward<[T; 2]> {
    type SnapForward = [@T; 2];
    fn snap_forward(self: @[T; 2]) -> [@T; 2] nopanic {
        let [e0, e1] = self;
        [e0, e1]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized3<T> of DojoTupleSnapForward<[T; 3]> {
    type SnapForward = [@T; 3];
    fn snap_forward(self: @[T; 3]) -> [@T; 3] nopanic {
        let [e0, e1, e2] = self;
        [e0, e1, e2]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized4<T> of DojoTupleSnapForward<[T; 4]> {
    type SnapForward = [@T; 4];
    fn snap_forward(self: @[T; 4]) -> [@T; 4] nopanic {
        let [e0, e1, e2, e3] = self;
        [e0, e1, e2, e3]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized5<T> of DojoTupleSnapForward<[T; 5]> {
    type SnapForward = [@T; 5];
    fn snap_forward(self: @[T; 5]) -> [@T; 5] nopanic {
        let [e0, e1, e2, e3, e4] = self;
        [e0, e1, e2, e3, e4]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized6<T> of DojoTupleSnapForward<[T; 6]> {
    type SnapForward = [@T; 6];
    fn snap_forward(self: @[T; 6]) -> [@T; 6] nopanic {
        let [e0, e1, e2, e3, e4, e5] = self;
        [e0, e1, e2, e3, e4, e5]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized7<T> of DojoTupleSnapForward<[T; 7]> {
    type SnapForward = [@T; 7];
    fn snap_forward(self: @[T; 7]) -> [@T; 7] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6] = self;
        [e0, e1, e2, e3, e4, e5, e6]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized8<T> of DojoTupleSnapForward<[T; 8]> {
    type SnapForward = [@T; 8];
    fn snap_forward(self: @[T; 8]) -> [@T; 8] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized9<T> of DojoTupleSnapForward<[T; 9]> {
    type SnapForward = [@T; 9];
    fn snap_forward(self: @[T; 9]) -> [@T; 9] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized10<T> of DojoTupleSnapForward<[T; 10]> {
    type SnapForward = [@T; 10];
    fn snap_forward(self: @[T; 10]) -> [@T; 10] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized11<T> of DojoTupleSnapForward<[T; 11]> {
    type SnapForward = [@T; 11];
    fn snap_forward(self: @[T; 11]) -> [@T; 11] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized12<T> of DojoTupleSnapForward<[T; 12]> {
    type SnapForward = [@T; 12];
    fn snap_forward(self: @[T; 12]) -> [@T; 12] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized13<T> of DojoTupleSnapForward<[T; 13]> {
    type SnapForward = [@T; 13];
    fn snap_forward(self: @[T; 13]) -> [@T; 13] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized14<T> of DojoTupleSnapForward<[T; 14]> {
    type SnapForward = [@T; 14];
    fn snap_forward(self: @[T; 14]) -> [@T; 14] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized15<T> of DojoTupleSnapForward<[T; 15]> {
    type SnapForward = [@T; 15];
    fn snap_forward(self: @[T; 15]) -> [@T; 15] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14]
    }
}

pub(crate) impl DojoTupleSnapForwardFixedSizedArraySized16<T> of DojoTupleSnapForward<[T; 16]> {
    type SnapForward = [@T; 16];
    fn snap_forward(self: @[T; 16]) -> [@T; 16] nopanic {
        let [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15] = self;
        [e0, e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13, e14, e15]
    }
}

pub(crate) impl DojoSnapRemoveFixedSizedArray<T, const N: usize> of DojoSnapRemove<[@T; N]> {
    type Result = [T; N];
}

pub(crate) impl DojoTuplePartialEqHelperBaseFixedSizedArray<
    T,
> of DojoTuplePartialEqHelper<[@T; 0]> {
    fn eq(lhs: [@T; 0], rhs: [@T; 0]) -> bool {
        true
    }
    fn ne(lhs: [@T; 0], rhs: [@T; 0]) -> bool {
        false
    }
}
