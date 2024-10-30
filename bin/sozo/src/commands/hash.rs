use anyhow::Result;
use clap::Args;
use dojo_world::contracts::naming::compute_selector_from_tag;
use starknet::core::types::Felt;
use starknet::core::utils::{get_selector_from_name, starknet_keccak};
use starknet_crypto::{poseidon_hash_many, poseidon_hash_single};
use tracing::trace;

#[derive(Debug, Args)]
pub struct HashArgs {
    #[arg(help = "Input to hash. It can be a comma separated list of inputs or a single input. \
                  The single input can be a dojo tag or a felt.")]
    pub input: String,
}

impl HashArgs {
    pub fn run(self) -> Result<Vec<String>> {
        trace!(args = ?self);

        if self.input.is_empty() {
            return Err(anyhow::anyhow!("Input is empty"));
        }

        if self.input.contains('-') {
            let selector = format!("{:#066x}", compute_selector_from_tag(&self.input));
            println!("Dojo selector from tag: {}", selector);
            return Ok(vec![selector.to_string()]);
        }

        // Selector in starknet is used for types, which must starts with a letter.
        if self.input.chars().next().map_or(false, |c| c.is_alphabetic()) {
            if self.input.len() > 32 {
                return Err(anyhow::anyhow!("Input is too long for a starknet selector"));
            }

            let selector = format!("{:#066x}", get_selector_from_name(&self.input)?);
            println!("Starknet selector: {}", selector);
            return Ok(vec![selector.to_string()]);
        }

        if !self.input.contains(',') {
            let felt = felt_from_str(&self.input)?;
            let poseidon = format!("{:#066x}", poseidon_hash_single(felt));
            let poseidon_array = format!("{:#066x}", poseidon_hash_many(&[felt]));
            let snkeccak = format!("{:#066x}", starknet_keccak(&felt.to_bytes_le()));

            println!("Poseidon single: {}", poseidon);
            println!("Poseidon array 1 value: {}", poseidon_array);
            println!("SnKeccak: {}", snkeccak);

            return Ok(vec![poseidon.to_string(), snkeccak.to_string()]);
        }

        let inputs: Vec<_> = self
            .input
            .split(',')
            .map(|s| felt_from_str(s.trim()).expect("Invalid felt value"))
            .collect();

        let poseidon = format!("{:#066x}", poseidon_hash_many(&inputs));
        println!("Poseidon many: {}", poseidon);

        Ok(vec![poseidon.to_string()])
    }
}

fn felt_from_str(s: &str) -> Result<Felt> {
    if s.starts_with("0x") {
        return Ok(Felt::from_hex(s)?);
    }

    Ok(Felt::from_dec_str(s)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_dojo_tag() {
        let args = HashArgs { input: "dojo_examples-actions".to_string() };
        let result = args.run();
        assert_eq!(
            result.unwrap(),
            ["0x040b6994c76da51db0c1dee2413641955fb3b15add8a35a2c605b1a050d225ab"]
        );
    }

    #[test]
    fn test_hash_single_felt() {
        let args = HashArgs { input: "0x1".to_string() };
        let result = args.run();
        assert_eq!(
            result.unwrap(),
            [
                "0x06d226d4c804cd74567f5ac59c6a4af1fe2a6eced19fb7560a9124579877da25",
                "0x00078cfed56339ea54962e72c37c7f588fc4f8e5bc173827ba75cb10a63a96a5"
            ]
        );
    }

    #[test]
    fn test_hash_starknet_selector() {
        let args = HashArgs { input: "dojo".to_string() };
        let result = args.run();
        assert_eq!(
            result.unwrap(),
            ["0x0120c91ffcb74234971d98abba5372798d16dfa5c6527911956861315c446e35"]
        );
    }

    #[test]
    fn test_hash_multiple_felts() {
        let args = HashArgs { input: "0x1,0x2,0x3".to_string() };
        let result = args.run();
        assert_eq!(
            result.unwrap(),
            ["0x02f0d8840bcf3bc629598d8a6cc80cb7c0d9e52d93dab244bbf9cd0dca0ad082"]
        );
    }

    #[test]
    fn test_hash_empty_input() {
        let args = HashArgs { input: "".to_string() };
        let result = args.run();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Input is empty");
    }

    #[test]
    fn test_hash_invalid_felt() {
        let args = HashArgs {
            input: "invalid too long to be a selector supported by starknet".to_string(),
        };
        assert!(args.run().is_err());
    }

    #[test]
    #[should_panic]
    fn test_hash_multiple_invalid_felts() {
        let args = HashArgs { input: "0x1,0x2,0x3,fhorihgorh".to_string() };

        let _ = args.run();
    }
}
