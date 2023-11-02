//! Deck struct and random card drawing methods.

// Core imports

use dict::{Felt252Dict, Felt252DictTrait};
use hash::HashStateTrait;
use nullable::{NullableTrait, nullable_from_box, match_nullable, FromNullableResult};
use poseidon::PoseidonTrait;
use traits::{Into, Drop};

/// Deck struct.
#[derive(Destruct)]
struct Deck {
    seed: felt252,
    owned: Felt252Dict<Nullable<bool>>,
    total: u32,
    remaining: u32,
    nonce: u8,
}

/// Errors module.
mod errors {
    const NO_CARD_LEFT: felt252 = 'Deck: no card left';
}

/// Trait to initialize, draw and discard a card from the Deck.
trait DeckTrait {
    /// Returns a new `Deck` struct.
    /// # Arguments
    /// * `seed` - A seed to initialize the deck.
    /// * `number` - The initial number of cards.
    /// # Returns
    /// * The initialized `Deck`.
    fn new(seed: felt252, number: u32, nonce: u8) -> Deck;
    /// Returns a card type after a draw.
    /// # Arguments
    /// * `self` - The Deck.
    /// # Returns
    /// * The card type.
    fn draw(ref self: Deck) -> u8;
    /// Returns a card to the deck.
    /// # Arguments
    /// * `self` - The Deck.
    /// * `card` - The card to discard.
    fn discard(ref self: Deck, card: u8);
    /// Set the cards status to owned, they are not drawable anymore.
    /// # Arguments
    /// * `self` - The Deck.
    /// * `cards` - The card to set.
    fn remove(ref self: Deck, cards: Span<u8>);
}

/// Implementation of the `DeckTrait` trait for the `Deck` struct.
impl DeckImpl of DeckTrait {
    #[inline(always)]
    fn new(seed: felt252, number: u32, nonce: u8) -> Deck {
        Deck { seed, owned: Default::default(), total: number, remaining: number, nonce }
    }

    fn draw(ref self: Deck) -> u8 {
        // [Check] Enough cards left.
        assert(self.remaining > 0, errors::NO_CARD_LEFT);
        // [Compute] Draw a random card from remaining not owned cards.
        let mut index: u32 = 0;
        loop {
            let mut state = PoseidonTrait::new();
            state = state.update(self.seed);
            state = state.update(self.nonce.into());
            state = state.update(index.into());
            let random: u256 = state.finalize().into();

            let card: u8 = (random % self.total.into() + 1).try_into().unwrap();
            let owned = match match_nullable(self.owned.get(card.into())) {
                FromNullableResult::Null => false,
                FromNullableResult::NotNull(status) => status.unbox(),
            };
            if !owned {
                self.owned.insert(card.into(), nullable_from_box(BoxTrait::new(true)));
                self.nonce += 1;
                self.remaining -= 1;
                break card;
            }
            index += 1;
        }
    }

    #[inline(always)]
    fn discard(ref self: Deck, card: u8) {
        self.remaining += 1;
        self.owned.insert(card.into(), nullable_from_box(BoxTrait::new(false)));
    }

    fn remove(ref self: Deck, mut cards: Span<u8>) {
        loop {
            match cards.pop_front() {
                Option::Some(card) => {
                    self.remaining -= 1;
                    self.owned.insert((*card).into(), nullable_from_box(BoxTrait::new(true)));
                },
                Option::None => { break; },
            };
        };
    }
}

#[cfg(test)]
mod tests {
    // Core imports

    use debug::PrintTrait;

    // Local imports

    use super::DeckTrait;

    // Constants

    const DECK_CARDS_NUMBER: u32 = 42;
    const DECK_SEED: felt252 = 'seed';

    #[test]
    #[available_gas(4_725_000)]
    fn test_deck_new_draw() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER, 0);
        assert(deck.total == DECK_CARDS_NUMBER, 'Wrong total');
        assert(deck.remaining == DECK_CARDS_NUMBER, 'Wrong remaining');
        assert(deck.draw() == 0x28, 'Wrong card 01');
        assert(deck.draw() == 0x1c, 'Wrong card 02');
        assert(deck.draw() == 0x03, 'Wrong card 03');
        assert(deck.draw() == 0x2a, 'Wrong card 04');
        assert(deck.draw() == 0x07, 'Wrong card 05');
        assert(deck.draw() == 0x13, 'Wrong card 06');
        assert(deck.draw() == 0x18, 'Wrong card 07');
        assert(deck.draw() == 0x14, 'Wrong card 08');
        assert(deck.draw() == 0x10, 'Wrong card 09');
        assert(deck.draw() == 0x21, 'Wrong card 10');
        assert(deck.draw() == 0x04, 'Wrong card 11');
        assert(deck.draw() == 0x24, 'Wrong card 12');
        assert(deck.draw() == 0x0f, 'Wrong card 13');
        assert(deck.draw() == 0x1b, 'Wrong card 14');
        assert(deck.draw() == 0x25, 'Wrong card 15');
        assert(deck.draw() == 0x19, 'Wrong card 16');
        assert(deck.draw() == 0x02, 'Wrong card 17');
        assert(deck.draw() == 0x11, 'Wrong card 18');
        assert(deck.draw() == 0x09, 'Wrong card 19');
        assert(deck.draw() == 0x0d, 'Wrong card 20');
        assert(deck.draw() == 0x0a, 'Wrong card 21');
        assert(deck.draw() == 0x15, 'Wrong card 22');
        assert(deck.draw() == 0x1e, 'Wrong card 23');
        assert(deck.draw() == 0x1d, 'Wrong card 24');
        assert(deck.draw() == 0x27, 'Wrong card 25');
        assert(deck.draw() == 0x16, 'Wrong card 26');
        assert(deck.draw() == 0x17, 'Wrong card 27');
        assert(deck.draw() == 0x01, 'Wrong card 28');
        assert(deck.draw() == 0x22, 'Wrong card 29');
        assert(deck.draw() == 0x26, 'Wrong card 30');
        assert(deck.draw() == 0x0c, 'Wrong card 31');
        assert(deck.draw() == 0x0e, 'Wrong card 32');
        assert(deck.draw() == 0x06, 'Wrong card 33');
        assert(deck.draw() == 0x20, 'Wrong card 34');
        assert(deck.draw() == 0x29, 'Wrong card 35');
        assert(deck.draw() == 0x08, 'Wrong card 36');
        assert(deck.draw() == 0x1f, 'Wrong card 37');
        assert(deck.draw() == 0x12, 'Wrong card 38');
        assert(deck.draw() == 0x23, 'Wrong card 39');
        assert(deck.draw() == 0x0b, 'Wrong card 40');
        assert(deck.draw() == 0x05, 'Wrong card 41');
        assert(deck.draw() == 0x1a, 'Wrong card 42');
        assert(deck.total == DECK_CARDS_NUMBER, 'Wrong total');
        assert(deck.remaining == 0, 'Wrong remaining');
    }

    #[test]
    #[available_gas(27_000)]
    #[should_panic(expected: ('Deck: no card left',))]
    fn test_deck_new_draw_revert_no_card_left() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER, 0);
        deck.remaining = 0;
        deck.draw();
    }

    #[test]
    #[available_gas(6_568_000)]
    fn test_deck_new_discard() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER, 0);
        loop {
            if deck.remaining == 0 {
                break;
            };
            deck.draw();
        };
        let card: u8 = 0x11;
        deck.discard(card);
        assert(deck.draw() == card, 'Wrong card');
    }

    #[test]
    #[available_gas(7_927_000)]
    fn test_deck_new_remove() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER, 0);
        let mut cards: Array<u8> = array![];
        let mut card: u8 = 1;
        loop {
            if card.into() > DECK_CARDS_NUMBER {
                break;
            };
            cards.append(card);
            card += 1;
        };
        deck.remove(cards.span());
        let card: u8 = 0x11;
        deck.discard(card);
        assert(deck.draw() == card, 'Wrong card');
    }
}
