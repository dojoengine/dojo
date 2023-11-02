//! Deck struct and random card drawing methods.

// Core imports

use dict::{Felt252Dict, Felt252DictTrait};
use hash::HashStateTrait;
use poseidon::PoseidonTrait;
use traits::{Into, Drop};

/// Deck struct.
#[derive(Destruct)]
struct Deck {
    seed: felt252,
    keys: Felt252Dict<felt252>,
    cards: Felt252Dict<u8>,
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
    fn new(seed: felt252, number: u32) -> Deck;
    /// Returns a card type after a draw.
    /// # Arguments
    /// * `self` - The Deck.
    /// # Returns
    /// * The card type.
    fn draw(ref self: Deck) -> u8;
    /// Returns a card into the deck, the card becomes drawable.
    /// # Arguments
    /// * `self` - The Deck.
    /// * `card` - The card to discard.
    fn discard(ref self: Deck, card: u8);
    /// Withdraw a card from the deck, the card is not drawable anymore.
    /// # Arguments
    /// * `self` - The Deck.
    /// * `card` - The card to withdraw.
    fn withdraw(ref self: Deck, card: u8);
    /// Remove the cards from the deck, they are not drawable anymore.
    /// # Arguments
    /// * `self` - The Deck.
    /// * `cards` - The card to set.
    fn remove(ref self: Deck, cards: Span<u8>);
}

/// Implementation of the `DeckTrait` trait for the `Deck` struct.
impl DeckImpl of DeckTrait {
    #[inline(always)]
    fn new(seed: felt252, number: u32) -> Deck {
        Deck {
            seed, cards: Default::default(), keys: Default::default(), remaining: number, nonce: 0
        }
    }

    #[inline(always)]
    fn draw(ref self: Deck) -> u8 {
        // [Check] Enough cards left.
        assert(self.remaining > 0, errors::NO_CARD_LEFT);
        // [Compute] Draw a random card from remainingcs cards.
        let mut state = PoseidonTrait::new();
        state = state.update(self.seed);
        state = state.update(self.nonce.into());
        state = state.update(self.remaining.into());
        let random: u256 = state.finalize().into();

        let key: felt252 = (random % self.remaining.into() + 1).try_into().unwrap();
        let mut card: u8 = self.cards.get(key);
        if 0 == card.into() {
            card = key.try_into().unwrap();
        }

        // [Compute] Remove card from the deck.
        self.withdraw(card);
        self.nonce += 1;
        card
    }

    #[inline(always)]
    fn discard(ref self: Deck, card: u8) {
        self.remaining += 1;
        self.cards.insert(self.remaining.into(), card);
    }

    #[inline(always)]
    fn withdraw(ref self: Deck, card: u8) {
        let mut key = self.keys.get(card.into());
        if key == 0 {
            key = card.into();
        }
        let latest_key: felt252 = self.remaining.into();
        if latest_key != key {
            let mut latest_card: u8 = self.cards.get(latest_key);
            if latest_card == 0 {
                latest_card = latest_key.try_into().unwrap();
            }
            self.cards.insert(key, latest_card);
            self.keys.insert(latest_card.into(), key);
        }
        self.remaining -= 1;
    }

    fn remove(ref self: Deck, mut cards: Span<u8>) {
        loop {
            match cards.pop_front() {
                Option::Some(card) => { self.withdraw(*card); },
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

    const DECK_CARDS_NUMBER: u32 = 5;
    const DECK_SEED: felt252 = 'SEED';

    #[test]
    #[available_gas(500_000)]
    fn test_deck_new_draw() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER);
        assert(deck.remaining == DECK_CARDS_NUMBER, 'Wrong remaining');
        assert(deck.draw() == 0x2, 'Wrong card 01');
        assert(deck.draw() == 0x4, 'Wrong card 02');
        assert(deck.draw() == 0x1, 'Wrong card 03');
        assert(deck.draw() == 0x5, 'Wrong card 04');
        assert(deck.draw() == 0x3, 'Wrong card 05');
        assert(deck.remaining == 0, 'Wrong remaining');
    }

    #[test]
    #[available_gas(500_000)]
    fn test_deck_new_withdraw() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER);
        deck.withdraw(0x2);
        assert(deck.draw() == 0x3, 'Wrong card 01');
        assert(deck.draw() == 0x1, 'Wrong card 02');
        assert(deck.draw() == 0x5, 'Wrong card 03');
        assert(deck.draw() == 0x4, 'Wrong card 04');
        assert(deck.remaining == 0, 'Wrong remaining');
    }

    #[test]
    #[available_gas(100_000)]
    #[should_panic(expected: ('Deck: no card left',))]
    fn test_deck_new_draw_revert_no_card_left() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER);
        deck.remaining = 0;
        deck.draw();
    }

    #[test]
    #[available_gas(600_000)]
    fn test_deck_new_discard() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER);
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
    #[available_gas(400_000)]
    fn test_deck_new_remove() {
        let mut deck = DeckTrait::new(DECK_SEED, DECK_CARDS_NUMBER);
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
