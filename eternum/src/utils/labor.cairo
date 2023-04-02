use eternum::utils::math::u128_div_remainder;
use eternum::utils::math::get_percentage_by_bp;

use eternum::constants::VAULT_BP;


fn get_harvestable_labor(harvest: u128) -> (u128, u128) {
    // calculate vault share
    let vault = get_percentage_by_bp(harvest, VAULT_BP);
    return u128_div_remainder(harvest, vault);
}
