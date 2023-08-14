pub mod account;
pub mod starknet;
pub mod world;

#[cfg(test)]
mod test {
    use dojo_world::environment::DojoMetadata;

    #[test]
    fn check_deserialization() {
        let metadata: DojoMetadata = toml::from_str(
            r#"
[env]
rpc_url = "http://localhost:5050/"
account_address = "0x03ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0"
private_key = "0x0300001800000000300000180000000000030000000000003006001800006600"
keystore_path = "test/"
keystore_password = "dojo"
world_address = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a"
        "#,
        )
        .unwrap();

        assert!(metadata.env.is_some());
        let env = metadata.env.unwrap();

        assert_eq!(env.rpc_url(), Some("http://localhost:5050/"));
        assert_eq!(
            env.account_address(),
            Some("0x03ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0")
        );
        assert_eq!(
            env.private_key(),
            Some("0x0300001800000000300000180000000000030000000000003006001800006600")
        );
        assert_eq!(env.keystore_path(), Some("test/"));
        assert_eq!(env.keystore_password(), Some("dojo"));
        assert_eq!(
            env.world_address(),
            Some("0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a")
        );
    }
}
