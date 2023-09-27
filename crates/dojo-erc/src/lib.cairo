mod token {
    mod erc20;
    mod erc20_components;
    mod erc721;

}

#[cfg(test)]
mod tests {
    mod constants;
    mod utils;

    mod erc20_tests;
    mod erc721_tests;
}
