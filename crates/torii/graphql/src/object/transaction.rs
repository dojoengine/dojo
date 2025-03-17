use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::{Name, Value};
use sqlx::{Pool, Sqlite, Row};
use std::collections::HashMap;

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{
    FUNCTION_CALL_TYPE_NAME, ID_COLUMN, TRANSACTION_HASH_COLUMN, TRANSACTION_NAMES,
    TRANSACTION_TABLE, TRANSACTION_TYPE_NAME, TRANSACTION_CALLS_TABLE,
};
use crate::mapping::{FUNCTION_CALL_MAPPING, TRANSACTION_MAPPING};
use crate::object::{extract, resolve_one};
use crate::query::value_mapping_from_row;

#[derive(Debug)]
pub struct FunctionCallObject;

impl BasicObject for FunctionCallObject {
    fn name(&self) -> (&str, &str) {
        ("functionCall", "functionCalls")
    }

    fn type_name(&self) -> &str {
        FUNCTION_CALL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &FUNCTION_CALL_MAPPING
    }
}

#[derive(Debug)]
pub struct TransactionObject;

impl BasicObject for TransactionObject {
    fn name(&self) -> (&str, &str) {
        TRANSACTION_NAMES
    }

    fn type_name(&self) -> &str {
        TRANSACTION_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }
}

impl ResolvableObject for TransactionObject {
    fn resolvers(&self) -> Vec<Field> {
        let resolve_one_with_calls = self.resolve_transaction_with_calls();
        let resolve_many_with_calls = self.resolve_transactions_with_calls();

        vec![resolve_one_with_calls, resolve_many_with_calls]
    }
}

impl TransactionObject {
    fn resolve_transaction_with_calls(&self) -> Field {
        let type_mapping = self.type_mapping().clone();
        let function_call_mapping = FUNCTION_CALL_MAPPING.clone();
        let table_name = TRANSACTION_TABLE.to_owned();
        let id_column = TRANSACTION_HASH_COLUMN.to_owned();
        let field_name = self.name().0.to_owned();
        let type_name = self.type_name().to_owned();
        
        Field::new(field_name, TypeRef::named_nn(type_name), move |ctx| {
            let type_mapping = type_mapping.clone();
            let function_call_mapping = function_call_mapping.clone();
            let table_name = table_name.clone();
            let id_column = id_column.clone();

            FieldFuture::new(async move {
                let pool = ctx.data::<Pool<Sqlite>>()?;
                
                // Extract the transaction hash from arguments
                let tx_hash: String = extract(ctx.args.as_index_map(), "transactionHash")?;
                
                // First, fetch the transaction
                let transaction_query = format!(
                    "SELECT * FROM [{}] WHERE {} = '{}'", 
                    table_name, id_column, tx_hash
                );
                let transaction = sqlx::query(&transaction_query)
                    .fetch_one(pool)
                    .await?;
                
                // Convert transaction row to GraphQL object
                let mut transaction_obj = value_mapping_from_row(&transaction, &type_mapping, false, true)?;
                
                // Then, fetch all calls for this transaction
                let calls_query = format!(
                    "SELECT * FROM [{}] WHERE transaction_hash = '{}'",
                    TRANSACTION_CALLS_TABLE, tx_hash
                );
                let calls = sqlx::query(&calls_query)
                    .fetch_all(pool)
                    .await?;
                
                // Convert each call row to a GraphQL object and add to a list
                let mut calls_list = Vec::new();
                for call in calls {
                    let call_obj = value_mapping_from_row(&call, &function_call_mapping, false, true)?;
                    calls_list.push(Value::Object(call_obj));
                }
                
                // Add the calls list as a field in the transaction object
                transaction_obj.insert(Name::new("calls"), Value::List(calls_list));
                
                Ok(Some(Value::Object(transaction_obj)))
            })
        })
        .argument(async_graphql::dynamic::InputValue::new(
            "transactionHash", 
            TypeRef::named_nn(TypeRef::ID)
        ))
    }

    fn resolve_transactions_with_calls(&self) -> Field {
        let type_mapping = self.type_mapping().clone();
        let function_call_mapping = FUNCTION_CALL_MAPPING.clone();
        let table_name = TRANSACTION_TABLE.to_owned();
        let field_name = self.name().1.to_owned();
        let type_name = self.type_name().to_owned();
        
        // Add pagination arguments
        let limit_arg = async_graphql::dynamic::InputValue::new(
            "limit", 
            TypeRef::named(TypeRef::INT)
        ).default_value(50);
        
        let offset_arg = async_graphql::dynamic::InputValue::new(
            "offset", 
            TypeRef::named(TypeRef::INT)
        ).default_value(0);

        let order_by_arg = async_graphql::dynamic::InputValue::new(
            "orderBy", 
            TypeRef::named(TypeRef::STRING)
        ).default_value("executed_at");

        let order_direction_arg = async_graphql::dynamic::InputValue::new(
            "orderDirection", 
            TypeRef::named(TypeRef::STRING)
        ).default_value("DESC");
        
        Field::new(field_name, TypeRef::named_list(type_name), move |ctx| {
            let type_mapping = type_mapping.clone();
            let function_call_mapping = function_call_mapping.clone();
            let table_name = table_name.clone();

            FieldFuture::new(async move {
                let pool = ctx.data::<Pool<Sqlite>>()?;
                
                // Extract pagination parameters
                let limit: u64 = extract(ctx.args.as_index_map(), "limit").unwrap_or(50);
                let offset: u64 = extract(ctx.args.as_index_map(), "offset").unwrap_or(0);
                let order_by: String = extract(ctx.args.as_index_map(), "orderBy").unwrap_or_else(|_| "executed_at".to_string());
                let order_direction: String = extract(ctx.args.as_index_map(), "orderDirection").unwrap_or_else(|_| "DESC".to_string());
                
                // Fetch transactions with pagination
                let transactions_query = format!(
                    "SELECT * FROM [{}] ORDER BY {} {} LIMIT {} OFFSET {}", 
                    table_name, order_by, order_direction, limit, offset
                );
                let transactions = sqlx::query(&transactions_query)
                    .fetch_all(pool)
                    .await?;
                
                // Create a list to hold all transaction objects
                let mut transaction_objects = Vec::new();
                
                // Process each transaction
                for transaction in transactions {
                    // Convert transaction row to GraphQL object
                    let mut transaction_obj = value_mapping_from_row(&transaction, &type_mapping, false, true)?;
                    
                    // Get the transaction hash for this transaction
                    let tx_hash = transaction.get::<String, _>(TRANSACTION_HASH_COLUMN);
                    
                    // Fetch all calls for this transaction
                    let calls_query = format!(
                        "SELECT * FROM [{}] WHERE transaction_hash = '{}'",
                        TRANSACTION_CALLS_TABLE, tx_hash
                    );
                    let calls = sqlx::query(&calls_query)
                        .fetch_all(pool)
                        .await?;
                    
                    // Convert each call row to a GraphQL object and add to a list
                    let mut calls_list = Vec::new();
                    for call in calls {
                        let call_obj = value_mapping_from_row(&call, &function_call_mapping, false, true)?;
                        calls_list.push(Value::Object(call_obj));
                    }
                    
                    // Add the calls list as a field in the transaction object
                    transaction_obj.insert(Name::new("calls"), Value::List(calls_list));
                    
                    // Add the transaction to our list
                    transaction_objects.push(Value::Object(transaction_obj));
                }
                
                Ok(Some(Value::List(transaction_objects)))
            })
        })
        .argument(limit_arg)
        .argument(offset_arg)
        .argument(order_by_arg)
        .argument(order_direction_arg)
    }
}
