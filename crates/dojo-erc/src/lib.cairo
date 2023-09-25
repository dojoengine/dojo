mod token {
    mod erc20;
    mod erc20_models;
    mod erc721;
    mod erc1155;

}

#[cfg(test)]
mod tests {
    mod constants;
    mod utils;

    mod erc20_tests;
    mod erc721_tests;
    mod erc1155_tests;
}
