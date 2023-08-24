#[cfg(test)]
mod transactions {

    use blockifier::state::state_api::StateReader;

    use crate::client::Client;
    use crate::utils::{addr, invoke_calldata, invoke_tx, ACCOUNT_ADDR, FEE_TKN_ADDR};

    #[test]
    fn state() {
        let mut client = Client::new();
        let s = client.state();
        let tkn_class = s.get_compiled_contract_class(&addr::class(FEE_TKN_ADDR));
        assert!(tkn_class.is_ok(), "tkn class missing");
        let acc_class = s.get_compiled_contract_class(&addr::class(ACCOUNT_ADDR));
        assert!(acc_class.is_ok(), "acc class missing");
        let tkn_contract = s.get_class_hash_at(addr::contract(FEE_TKN_ADDR));
        assert!(tkn_contract.unwrap() == addr::class(FEE_TKN_ADDR), "tkn contract incorrect class");
        let acc_contract = s.get_class_hash_at(addr::contract(ACCOUNT_ADDR));
        assert!(acc_contract.unwrap() == addr::class(ACCOUNT_ADDR), "acc contract incorrect class");
    }

    #[test]
    fn txn() {
        let mut client = Client::new();

        let txn = invoke_tx(
            ACCOUNT_ADDR,
            invoke_calldata(FEE_TKN_ADDR, "balanceOf", vec!["0x1", ACCOUNT_ADDR]),
            None,
            "1",
        );

        let res = client.execute(txn);

        assert!(res.is_ok(), "Transaction failed");
        // if let Ok(exec_info) = res {
        //     assert!(!exec_info.is_reverted(), "Transaction reverted");
        //     assert!(exec_info.execute_call_info.is_some(), "No execution call info");
        // }
    }
}
