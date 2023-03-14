// Statistics levels of an Entity. This could be a Player or a Beast.

#[derive(Component)]
struct Statistics {
    // Physical
    Strength: felt,
    Dexterity: felt,
    Vitality: felt,
    // Mental
    Intelligence: felt,
    Wisdom: felt,
    Charisma: felt,
}