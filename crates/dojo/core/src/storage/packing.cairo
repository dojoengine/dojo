use core::array::{ArrayTrait, SpanTrait};
use core::num::traits::Pow;
use core::option::OptionTrait;
use core::traits::{Into, TryInto};

pub const PACKING_MAX_BITS: u8 = 251;
pub const PACKING_MAX_BITS_USIZE: usize = 251;

pub fn pack(
    ref packed: Array<felt252>, ref unpacked: Span<felt252>, offset: u32, ref layout: Span<u8>,
) {
    assert((unpacked.len() - offset) >= layout.len(), 'mismatched input lens');
    let mut packing: felt252 = 0x0;
    let mut internal_offset: usize = 0x0;
    let mut index = offset;

    for l in layout {
        pack_inner(unpacked.at(index), (*l).into(), ref packing, ref internal_offset, ref packed);
        index += 1;
    }

    packed.append(packing);
}

pub fn calculate_packed_size(ref layout: Span<u8>) -> usize {
    let mut size = 1;
    let mut partial = 0_usize;

    for l in layout {
        let item_size: usize = (*l).into();
        partial += item_size;
        if (partial > PACKING_MAX_BITS_USIZE) {
            size += 1;
            partial = item_size;
        }
    }

    size
}

pub fn unpack(ref unpacked: Array<felt252>, ref packed: Span<felt252>, ref layout: Span<u8>) {
    let mut unpacking: felt252 = 0x0;
    let mut offset: usize = PACKING_MAX_BITS_USIZE;

    for l in layout {
        match unpack_inner((*l).into(), ref packed, ref unpacking, ref offset) {
            Some(u) => { unpacked.append(u); },
            None => {
                // Layout value was successfully popped,
                // we are then expecting an unpacked value.
                core::panic_with_felt252('Unpack inner failed');
            },
        }
    }
}

/// Pack the proposal fields into a single felt252.
pub fn pack_inner(
    self: @felt252,
    size: usize,
    ref packing: felt252,
    ref packing_offset: usize,
    ref packed: Array<felt252>,
) {
    assert(packing_offset <= PACKING_MAX_BITS_USIZE, 'Invalid packing offset');
    assert(size <= PACKING_MAX_BITS_USIZE, 'Invalid layout size');

    // Cannot use all 252 bits because some bit arrangements (eg. 11111...11111) are not valid
    // felt252 values.                                            ^-252 times-^
    // Thus only 251 bits are used.
    // One could optimize by some conditional alignment mechanism, but it would be an at most 1/252
    // space-wise improvement.
    let remaining_bits: usize = PACKING_MAX_BITS_USIZE - packing_offset;

    // If we have less remaining bits than the current item size,
    // Finalize the current `packing` felt and move to the next felt.
    if remaining_bits < size {
        packed.append(packing);
        packing = *self;
        packing_offset = size;
        return;
    }

    // Easier to work on u256 rather than felt252.
    let self_256: u256 = (*self).into();

    // Pack item into the `packing` felt.
    let mut packing_256: u256 = packing.into() | shl(self_256, packing_offset);
    packing = packing_256.try_into().unwrap();
    packing_offset += size;
}

pub fn unpack_inner(
    size: usize, ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: usize,
) -> Option<felt252> {
    let remaining_bits: usize = PACKING_MAX_BITS_USIZE - unpacking_offset;

    // If less remaining bits than size, we move to the next
    // felt for unpacking.
    if remaining_bits < size {
        if let Some(val) = packed.pop_front() {
            unpacking = *val;
            unpacking_offset = size;

            // If we are unpacking a full felt.
            if (size == PACKING_MAX_BITS_USIZE) {
                return Some(unpacking);
            }

            let val_256: u256 = (*val).into();
            let result = val_256 & *POW_2_MINUS_ONE.span().at(size);
            return result.try_into();
        }

        return None;
    }

    let mut unpacking_256: u256 = unpacking.into();
    let result = *POW_2_MINUS_ONE.span().at(size) & shr(unpacking_256, unpacking_offset);
    unpacking_offset = unpacking_offset + size;
    return result.try_into();
}

#[inline(always)]
pub fn shl(x: u256, n: usize) -> u256 {
    x * *POW_2.span().at(n)
}

#[inline(always)]
pub fn shr(x: u256, n: usize) -> u256 {
    x / *POW_2.span().at(n)
}

#[inline(always)]
pub fn pow2_const(n: usize) -> u256 {
    *POW_2.span().at(n)
}

// pre-compute shl(1, size) - 1 = 2^size - 1
pub const POW_2_MINUS_ONE: [u256; 256] = [
    2_u256.pow(0) - 1, 2_u256.pow(1) - 1, 2_u256.pow(2) - 1, 2_u256.pow(3) - 1, 2_u256.pow(4) - 1,
    2_u256.pow(5) - 1, 2_u256.pow(6) - 1, 2_u256.pow(7) - 1, 2_u256.pow(8) - 1, 2_u256.pow(9) - 1,
    2_u256.pow(10) - 1, 2_u256.pow(11) - 1, 2_u256.pow(12) - 1, 2_u256.pow(13) - 1,
    2_u256.pow(14) - 1, 2_u256.pow(15) - 1, 2_u256.pow(16) - 1, 2_u256.pow(17) - 1,
    2_u256.pow(18) - 1, 2_u256.pow(19) - 1, 2_u256.pow(20) - 1, 2_u256.pow(21) - 1,
    2_u256.pow(22) - 1, 2_u256.pow(23) - 1, 2_u256.pow(24) - 1, 2_u256.pow(25) - 1,
    2_u256.pow(26) - 1, 2_u256.pow(27) - 1, 2_u256.pow(28) - 1, 2_u256.pow(29) - 1,
    2_u256.pow(30) - 1, 2_u256.pow(31) - 1, 2_u256.pow(32) - 1, 2_u256.pow(33) - 1,
    2_u256.pow(34) - 1, 2_u256.pow(35) - 1, 2_u256.pow(36) - 1, 2_u256.pow(37) - 1,
    2_u256.pow(38) - 1, 2_u256.pow(39) - 1, 2_u256.pow(40) - 1, 2_u256.pow(41) - 1,
    2_u256.pow(42) - 1, 2_u256.pow(43) - 1, 2_u256.pow(44) - 1, 2_u256.pow(45) - 1,
    2_u256.pow(46) - 1, 2_u256.pow(47) - 1, 2_u256.pow(48) - 1, 2_u256.pow(49) - 1,
    2_u256.pow(50) - 1, 2_u256.pow(51) - 1, 2_u256.pow(52) - 1, 2_u256.pow(53) - 1,
    2_u256.pow(54) - 1, 2_u256.pow(55) - 1, 2_u256.pow(56) - 1, 2_u256.pow(57) - 1,
    2_u256.pow(58) - 1, 2_u256.pow(59) - 1, 2_u256.pow(60) - 1, 2_u256.pow(61) - 1,
    2_u256.pow(62) - 1, 2_u256.pow(63) - 1, 2_u256.pow(64) - 1, 2_u256.pow(65) - 1,
    2_u256.pow(66) - 1, 2_u256.pow(67) - 1, 2_u256.pow(68) - 1, 2_u256.pow(69) - 1,
    2_u256.pow(70) - 1, 2_u256.pow(71) - 1, 2_u256.pow(72) - 1, 2_u256.pow(73) - 1,
    2_u256.pow(74) - 1, 2_u256.pow(75) - 1, 2_u256.pow(76) - 1, 2_u256.pow(77) - 1,
    2_u256.pow(78) - 1, 2_u256.pow(79) - 1, 2_u256.pow(80) - 1, 2_u256.pow(81) - 1,
    2_u256.pow(82) - 1, 2_u256.pow(83) - 1, 2_u256.pow(84) - 1, 2_u256.pow(85) - 1,
    2_u256.pow(86) - 1, 2_u256.pow(87) - 1, 2_u256.pow(88) - 1, 2_u256.pow(89) - 1,
    2_u256.pow(90) - 1, 2_u256.pow(91) - 1, 2_u256.pow(92) - 1, 2_u256.pow(93) - 1,
    2_u256.pow(94) - 1, 2_u256.pow(95) - 1, 2_u256.pow(96) - 1, 2_u256.pow(97) - 1,
    2_u256.pow(98) - 1, 2_u256.pow(99) - 1, 2_u256.pow(100) - 1, 2_u256.pow(101) - 1,
    2_u256.pow(102) - 1, 2_u256.pow(103) - 1, 2_u256.pow(104) - 1, 2_u256.pow(105) - 1,
    2_u256.pow(106) - 1, 2_u256.pow(107) - 1, 2_u256.pow(108) - 1, 2_u256.pow(109) - 1,
    2_u256.pow(110) - 1, 2_u256.pow(111) - 1, 2_u256.pow(112) - 1, 2_u256.pow(113) - 1,
    2_u256.pow(114) - 1, 2_u256.pow(115) - 1, 2_u256.pow(116) - 1, 2_u256.pow(117) - 1,
    2_u256.pow(118) - 1, 2_u256.pow(119) - 1, 2_u256.pow(120) - 1, 2_u256.pow(121) - 1,
    2_u256.pow(122) - 1, 2_u256.pow(123) - 1, 2_u256.pow(124) - 1, 2_u256.pow(125) - 1,
    2_u256.pow(126) - 1, 2_u256.pow(127) - 1, 2_u256.pow(128) - 1, 2_u256.pow(129) - 1,
    2_u256.pow(130) - 1, 2_u256.pow(131) - 1, 2_u256.pow(132) - 1, 2_u256.pow(133) - 1,
    2_u256.pow(134) - 1, 2_u256.pow(135) - 1, 2_u256.pow(136) - 1, 2_u256.pow(137) - 1,
    2_u256.pow(138) - 1, 2_u256.pow(139) - 1, 2_u256.pow(140) - 1, 2_u256.pow(141) - 1,
    2_u256.pow(142) - 1, 2_u256.pow(143) - 1, 2_u256.pow(144) - 1, 2_u256.pow(145) - 1,
    2_u256.pow(146) - 1, 2_u256.pow(147) - 1, 2_u256.pow(148) - 1, 2_u256.pow(149) - 1,
    2_u256.pow(150) - 1, 2_u256.pow(151) - 1, 2_u256.pow(152) - 1, 2_u256.pow(153) - 1,
    2_u256.pow(154) - 1, 2_u256.pow(155) - 1, 2_u256.pow(156) - 1, 2_u256.pow(157) - 1,
    2_u256.pow(158) - 1, 2_u256.pow(159) - 1, 2_u256.pow(160) - 1, 2_u256.pow(161) - 1,
    2_u256.pow(162) - 1, 2_u256.pow(163) - 1, 2_u256.pow(164) - 1, 2_u256.pow(165) - 1,
    2_u256.pow(166) - 1, 2_u256.pow(167) - 1, 2_u256.pow(168) - 1, 2_u256.pow(169) - 1,
    2_u256.pow(170) - 1, 2_u256.pow(171) - 1, 2_u256.pow(172) - 1, 2_u256.pow(173) - 1,
    2_u256.pow(174) - 1, 2_u256.pow(175) - 1, 2_u256.pow(176) - 1, 2_u256.pow(177) - 1,
    2_u256.pow(178) - 1, 2_u256.pow(179) - 1, 2_u256.pow(180) - 1, 2_u256.pow(181) - 1,
    2_u256.pow(182) - 1, 2_u256.pow(183) - 1, 2_u256.pow(184) - 1, 2_u256.pow(185) - 1,
    2_u256.pow(186) - 1, 2_u256.pow(187) - 1, 2_u256.pow(188) - 1, 2_u256.pow(189) - 1,
    2_u256.pow(190) - 1, 2_u256.pow(191) - 1, 2_u256.pow(192) - 1, 2_u256.pow(193) - 1,
    2_u256.pow(194) - 1, 2_u256.pow(195) - 1, 2_u256.pow(196) - 1, 2_u256.pow(197) - 1,
    2_u256.pow(198) - 1, 2_u256.pow(199) - 1, 2_u256.pow(200) - 1, 2_u256.pow(201) - 1,
    2_u256.pow(202) - 1, 2_u256.pow(203) - 1, 2_u256.pow(204) - 1, 2_u256.pow(205) - 1,
    2_u256.pow(206) - 1, 2_u256.pow(207) - 1, 2_u256.pow(208) - 1, 2_u256.pow(209) - 1,
    2_u256.pow(210) - 1, 2_u256.pow(211) - 1, 2_u256.pow(212) - 1, 2_u256.pow(213) - 1,
    2_u256.pow(214) - 1, 2_u256.pow(215) - 1, 2_u256.pow(216) - 1, 2_u256.pow(217) - 1,
    2_u256.pow(218) - 1, 2_u256.pow(219) - 1, 2_u256.pow(220) - 1, 2_u256.pow(221) - 1,
    2_u256.pow(222) - 1, 2_u256.pow(223) - 1, 2_u256.pow(224) - 1, 2_u256.pow(225) - 1,
    2_u256.pow(226) - 1, 2_u256.pow(227) - 1, 2_u256.pow(228) - 1, 2_u256.pow(229) - 1,
    2_u256.pow(230) - 1, 2_u256.pow(231) - 1, 2_u256.pow(232) - 1, 2_u256.pow(233) - 1,
    2_u256.pow(234) - 1, 2_u256.pow(235) - 1, 2_u256.pow(236) - 1, 2_u256.pow(237) - 1,
    2_u256.pow(238) - 1, 2_u256.pow(239) - 1, 2_u256.pow(240) - 1, 2_u256.pow(241) - 1,
    2_u256.pow(242) - 1, 2_u256.pow(243) - 1, 2_u256.pow(244) - 1, 2_u256.pow(245) - 1,
    2_u256.pow(246) - 1, 2_u256.pow(247) - 1, 2_u256.pow(248) - 1, 2_u256.pow(249) - 1,
    2_u256.pow(250) - 1, 2_u256.pow(251) - 1, 2_u256.pow(252) - 1, 2_u256.pow(253) - 1,
    2_u256.pow(254) - 1, 2_u256.pow(255) - 1,
];

pub const POW_2: [u256; 256] = [
    2_u256.pow(0), 2_u256.pow(1), 2_u256.pow(2), 2_u256.pow(3), 2_u256.pow(4), 2_u256.pow(5),
    2_u256.pow(6), 2_u256.pow(7), 2_u256.pow(8), 2_u256.pow(9), 2_u256.pow(10), 2_u256.pow(11),
    2_u256.pow(12), 2_u256.pow(13), 2_u256.pow(14), 2_u256.pow(15), 2_u256.pow(16), 2_u256.pow(17),
    2_u256.pow(18), 2_u256.pow(19), 2_u256.pow(20), 2_u256.pow(21), 2_u256.pow(22), 2_u256.pow(23),
    2_u256.pow(24), 2_u256.pow(25), 2_u256.pow(26), 2_u256.pow(27), 2_u256.pow(28), 2_u256.pow(29),
    2_u256.pow(30), 2_u256.pow(31), 2_u256.pow(32), 2_u256.pow(33), 2_u256.pow(34), 2_u256.pow(35),
    2_u256.pow(36), 2_u256.pow(37), 2_u256.pow(38), 2_u256.pow(39), 2_u256.pow(40), 2_u256.pow(41),
    2_u256.pow(42), 2_u256.pow(43), 2_u256.pow(44), 2_u256.pow(45), 2_u256.pow(46), 2_u256.pow(47),
    2_u256.pow(48), 2_u256.pow(49), 2_u256.pow(50), 2_u256.pow(51), 2_u256.pow(52), 2_u256.pow(53),
    2_u256.pow(54), 2_u256.pow(55), 2_u256.pow(56), 2_u256.pow(57), 2_u256.pow(58), 2_u256.pow(59),
    2_u256.pow(60), 2_u256.pow(61), 2_u256.pow(62), 2_u256.pow(63), 2_u256.pow(64), 2_u256.pow(65),
    2_u256.pow(66), 2_u256.pow(67), 2_u256.pow(68), 2_u256.pow(69), 2_u256.pow(70), 2_u256.pow(71),
    2_u256.pow(72), 2_u256.pow(73), 2_u256.pow(74), 2_u256.pow(75), 2_u256.pow(76), 2_u256.pow(77),
    2_u256.pow(78), 2_u256.pow(79), 2_u256.pow(80), 2_u256.pow(81), 2_u256.pow(82), 2_u256.pow(83),
    2_u256.pow(84), 2_u256.pow(85), 2_u256.pow(86), 2_u256.pow(87), 2_u256.pow(88), 2_u256.pow(89),
    2_u256.pow(90), 2_u256.pow(91), 2_u256.pow(92), 2_u256.pow(93), 2_u256.pow(94), 2_u256.pow(95),
    2_u256.pow(96), 2_u256.pow(97), 2_u256.pow(98), 2_u256.pow(99), 2_u256.pow(100),
    2_u256.pow(101), 2_u256.pow(102), 2_u256.pow(103), 2_u256.pow(104), 2_u256.pow(105),
    2_u256.pow(106), 2_u256.pow(107), 2_u256.pow(108), 2_u256.pow(109), 2_u256.pow(110),
    2_u256.pow(111), 2_u256.pow(112), 2_u256.pow(113), 2_u256.pow(114), 2_u256.pow(115),
    2_u256.pow(116), 2_u256.pow(117), 2_u256.pow(118), 2_u256.pow(119), 2_u256.pow(120),
    2_u256.pow(121), 2_u256.pow(122), 2_u256.pow(123), 2_u256.pow(124), 2_u256.pow(125),
    2_u256.pow(126), 2_u256.pow(127), 2_u256.pow(128), 2_u256.pow(129), 2_u256.pow(130),
    2_u256.pow(131), 2_u256.pow(132), 2_u256.pow(133), 2_u256.pow(134), 2_u256.pow(135),
    2_u256.pow(136), 2_u256.pow(137), 2_u256.pow(138), 2_u256.pow(139), 2_u256.pow(140),
    2_u256.pow(141), 2_u256.pow(142), 2_u256.pow(143), 2_u256.pow(144), 2_u256.pow(145),
    2_u256.pow(146), 2_u256.pow(147), 2_u256.pow(148), 2_u256.pow(149), 2_u256.pow(150),
    2_u256.pow(151), 2_u256.pow(152), 2_u256.pow(153), 2_u256.pow(154), 2_u256.pow(155),
    2_u256.pow(156), 2_u256.pow(157), 2_u256.pow(158), 2_u256.pow(159), 2_u256.pow(160),
    2_u256.pow(161), 2_u256.pow(162), 2_u256.pow(163), 2_u256.pow(164), 2_u256.pow(165),
    2_u256.pow(166), 2_u256.pow(167), 2_u256.pow(168), 2_u256.pow(169), 2_u256.pow(170),
    2_u256.pow(171), 2_u256.pow(172), 2_u256.pow(173), 2_u256.pow(174), 2_u256.pow(175),
    2_u256.pow(176), 2_u256.pow(177), 2_u256.pow(178), 2_u256.pow(179), 2_u256.pow(180),
    2_u256.pow(181), 2_u256.pow(182), 2_u256.pow(183), 2_u256.pow(184), 2_u256.pow(185),
    2_u256.pow(186), 2_u256.pow(187), 2_u256.pow(188), 2_u256.pow(189), 2_u256.pow(190),
    2_u256.pow(191), 2_u256.pow(192), 2_u256.pow(193), 2_u256.pow(194), 2_u256.pow(195),
    2_u256.pow(196), 2_u256.pow(197), 2_u256.pow(198), 2_u256.pow(199), 2_u256.pow(200),
    2_u256.pow(201), 2_u256.pow(202), 2_u256.pow(203), 2_u256.pow(204), 2_u256.pow(205),
    2_u256.pow(206), 2_u256.pow(207), 2_u256.pow(208), 2_u256.pow(209), 2_u256.pow(210),
    2_u256.pow(211), 2_u256.pow(212), 2_u256.pow(213), 2_u256.pow(214), 2_u256.pow(215),
    2_u256.pow(216), 2_u256.pow(217), 2_u256.pow(218), 2_u256.pow(219), 2_u256.pow(220),
    2_u256.pow(221), 2_u256.pow(222), 2_u256.pow(223), 2_u256.pow(224), 2_u256.pow(225),
    2_u256.pow(226), 2_u256.pow(227), 2_u256.pow(228), 2_u256.pow(229), 2_u256.pow(230),
    2_u256.pow(231), 2_u256.pow(232), 2_u256.pow(233), 2_u256.pow(234), 2_u256.pow(235),
    2_u256.pow(236), 2_u256.pow(237), 2_u256.pow(238), 2_u256.pow(239), 2_u256.pow(240),
    2_u256.pow(241), 2_u256.pow(242), 2_u256.pow(243), 2_u256.pow(244), 2_u256.pow(245),
    2_u256.pow(246), 2_u256.pow(247), 2_u256.pow(248), 2_u256.pow(249), 2_u256.pow(250),
    2_u256.pow(251), 2_u256.pow(252), 2_u256.pow(253), 2_u256.pow(254), 2_u256.pow(255),
];
