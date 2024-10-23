use cainome::parser::tokens::Composite;

use super::get_namespace_and_path;
use crate::error::BindgenResult;
use crate::plugins::{BindgenModelGenerator, Buffer};

const ERC_TORII_TPL: &str = "// Type definition for ERC__Balance struct
export type ERC__Type = 'ERC20' | 'ERC721';
export interface ERC__Balance {
    fieldOrder: string[];
    balance: string;
    type: string;
    tokenMetadata: ERC__Token;
}
export interface ERC__Token {
    fieldOrder: string[];
    name: string;
    symbol: string;
    tokenId: string;
    decimals: string;
    contractAddress: string;
}
export interface ERC__Transfer {
    fieldOrder: string[];
    from: string;
    to: string;
    amount: string;
    type: string;
    executedAt: string;
    tokenMetadata: ERC__Token;
    transactionHash: string;
}";
const ERC_TORII_TYPES: &str = "\n\t\tERC__Balance: ERC__Balance,\n\t\tERC__Token: \
                               ERC__Token,\n\t\tERC__Transfer: ERC__Transfer,";
const ERC_TORII_INIT: &str = "
\t\tERC__Balance: {
\t\t\tfieldorder: ['balance', 'type', 'tokenmetadata'],
\t\t\tbalance: '',
\t\t\ttype: 'ERC20',
\t\t\ttokenMetadata: {
\t\t\t\tfieldorder: ['name', 'symbol', 'tokenId', 'decimals', 'contractAddress'],
\t\t\t\tname: '',
\t\t\t\tsymbol: '',
\t\t\t\ttokenId: '',
\t\t\t\tdecimals: '',
\t\t\t\tcontractAddress: '',
\t\t\t},
\t\t},
\t\tERC__Token: {
\t\t\tfieldOrder: ['name', 'symbol', 'tokenId', 'decimals', 'contractAddress'],
\t\t\tname: '',
\t\t\tsymbol: '',
\t\t\ttokenId: '',
\t\t\tdecimals: '',
\t\t\tcontractAddress: '',
\t\t},
\t\tERC__Transfer: {
\t\t\tfieldOrder: ['from', 'to', 'amount', 'type', 'executed', 'tokenMetadata'],
\t\t\tfrom: '',
\t\t\tto: '',
\t\t\tamount: '',
\t\t\ttype: 'ERC20',
\t\t\texecutedAt: '',
\t\t\ttokenMetadata: {
\t\t\t\tfieldOrder: ['name', 'symbol', 'tokenId', 'decimals', 'contractAddress'],
\t\t\t\tname: '',
\t\t\t\tsymbol: '',
\t\t\t\ttokenId: '',
\t\t\t\tdecimals: '',
\t\t\t\tcontractAddress: '',
\t\t\t},
\t\t\ttransactionHash: '',
\t\t},
";

pub(crate) struct TsErcGenerator;

impl TsErcGenerator {
    fn add_schema_type(&self, buffer: &mut Buffer, token: &Composite) {
        let (_, namespace, _) = get_namespace_and_path(token);
        let schema_type = format!("export interface {namespace}SchemaType extends SchemaType");
        if buffer.has(&schema_type) {
            if buffer.has(ERC_TORII_TYPES) {
                return;
            }

            buffer.insert_after(ERC_TORII_TYPES.to_owned(), &schema_type, ",", 2);
        }
    }

    fn add_schema_type_init(&self, buffer: &mut Buffer, token: &Composite) {
        let (_, namespace, _) = get_namespace_and_path(token);
        let const_type = format!("export const schema: {namespace}SchemaType");
        if buffer.has(&const_type) {
            if buffer.has(ERC_TORII_INIT) {
                return;
            }
            buffer.insert_after(ERC_TORII_INIT.to_owned(), &const_type, ",", 2);
        }
    }
}

impl BindgenModelGenerator for TsErcGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        if buffer.has(ERC_TORII_TPL) {
            return Ok(String::new());
        }

        // As this generator is separated from schema.rs we need to check if schema is present in
        // buffer and also adding torii types to schema to query this data through grpc
        self.add_schema_type(buffer, token);
        self.add_schema_type_init(buffer, token);

        Ok(ERC_TORII_TPL.to_owned())
    }
}
