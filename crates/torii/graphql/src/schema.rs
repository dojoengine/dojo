use anyhow::Result;
use async_graphql::dynamic::{Object, Scalar, Schema, Subscription, Union};
use dojo_types::schema::Ty;
use sqlx::SqlitePool;
use torii_sqlite::types::Model;

use super::object::connection::page_info::PageInfoObject;
use super::object::entity::EntityObject;
use super::object::event::EventObject;
use super::object::model_data::ModelDataObject;
use super::types::ScalarType;
use super::utils;
use crate::constants::{
    ERC20_TYPE_NAME, ERC721_TYPE_NAME, QUERY_TYPE_NAME, SUBSCRIPTION_TYPE_NAME, TOKEN_TYPE_NAME,
    EMPTY_TYPE_NAME,
};
use crate::object::controller::ControllerObject;
use crate::object::erc::erc_token::{Erc20TokenObject, Erc721TokenObject};
use crate::object::erc::token_balance::ErcBalanceObject;
use crate::object::erc::token_transfer::ErcTransferObject;
use crate::object::event_message::EventMessageObject;
use crate::object::metadata::content::ContentObject;
use crate::object::metadata::social::SocialObject;
use crate::object::metadata::MetadataObject;
use crate::object::model::ModelObject;
use crate::object::transaction::TransactionObject;
use crate::object::ObjectVariant;
use crate::query::build_type_mapping;
use crate::object::empty::EmptyObject;

// The graphql schema is built dynamically at runtime, this is because we won't know the schema of
// the models until runtime. There are however, predefined objects such as entities and
// events, their schema is known but we generate them dynamically as well because async-graphql
// does not allow mixing of static and dynamic schemas.
pub async fn build_schema(pool: &SqlitePool) -> Result<Schema> {
    // build world gql objects
    let (objects, unions) = build_objects(pool).await?;

    let mut schema_builder = Schema::build(QUERY_TYPE_NAME, None, Some(SUBSCRIPTION_TYPE_NAME));
    //? why we need to provide QUERY_TYPE_NAME object here when its already passed to Schema?
    let mut query_root = Object::new(QUERY_TYPE_NAME);
    let mut subscription_root = Subscription::new(SUBSCRIPTION_TYPE_NAME);

    // register model data unions
    for union in unions {
        schema_builder = schema_builder.register(union);
    }

    // register default scalars
    for scalar_type in ScalarType::all().iter() {
        schema_builder = schema_builder.register(Scalar::new(scalar_type));
    }

    // register objects
    for object in &objects {
        match object {
            ObjectVariant::Basic(object) => {
                // register objects
                for inner_object in object.objects() {
                    schema_builder = schema_builder.register(inner_object)
                }
            }
            ObjectVariant::Resolvable(object) => {
                // register objects
                for inner_object in object.objects() {
                    schema_builder = schema_builder.register(inner_object)
                }

                // register resolvers
                for resolver in object.resolvers() {
                    query_root = query_root.field(resolver);
                }

                // register connection types, relay
                if let Some(conn_objects) = object.connection_objects() {
                    for conn in conn_objects {
                        schema_builder = schema_builder.register(conn);
                    }
                }

                // register enum objects
                if let Some(input_objects) = object.enum_objects() {
                    for input in input_objects {
                        schema_builder = schema_builder.register(input);
                    }
                }

                // register input objects, whereInput and orderBy
                if let Some(input_objects) = object.input_objects() {
                    for input in input_objects {
                        schema_builder = schema_builder.register(input);
                    }
                }

                // register subscription
                if let Some(subscriptions) = object.subscriptions() {
                    for sub in subscriptions {
                        subscription_root = subscription_root.field(sub);
                    }
                }
            }
        }
    }

    schema_builder
        .register(query_root)
        .register(subscription_root)
        .data(pool.clone())
        .finish()
        .map_err(|e| e.into())
}

async fn build_objects(pool: &SqlitePool) -> Result<(Vec<ObjectVariant>, Vec<Union>)> {
    let mut conn = pool.acquire().await?;
    let models: Vec<Model> = sqlx::query_as("SELECT * FROM models").fetch_all(&mut *conn).await?;

    // predefined objects
    let mut objects: Vec<ObjectVariant> = vec![
        ObjectVariant::Resolvable(Box::new(EntityObject)),
        ObjectVariant::Resolvable(Box::new(EventMessageObject)),
        ObjectVariant::Resolvable(Box::new(EventObject)),
        ObjectVariant::Resolvable(Box::new(MetadataObject)),
        ObjectVariant::Resolvable(Box::new(ModelObject)),
        ObjectVariant::Resolvable(Box::new(TransactionObject)),
        ObjectVariant::Resolvable(Box::new(ErcBalanceObject)),
        ObjectVariant::Resolvable(Box::new(ErcTransferObject)),
        ObjectVariant::Basic(Box::new(SocialObject)),
        ObjectVariant::Basic(Box::new(ContentObject)),
        ObjectVariant::Basic(Box::new(PageInfoObject)),
        ObjectVariant::Basic(Box::new(Erc721TokenObject)),
        ObjectVariant::Basic(Box::new(Erc20TokenObject)),
        ObjectVariant::Basic(Box::new(EmptyObject)),
    ];

    // model union object
    let mut unions: Vec<Union> = Vec::new();
    let mut model_union = Union::new("ModelUnion");

    // erc_token union object
    let erc_token_union =
        Union::new(TOKEN_TYPE_NAME).possible_type(ERC20_TYPE_NAME).possible_type(ERC721_TYPE_NAME);

    unions.push(erc_token_union);

    // model data objects
    for model in &models {
        let schema: Ty = serde_json::from_str(&model.schema)
            .map_err(|e| anyhow::anyhow!(format!("Failed to parse model schema: {e}")))?;
        let type_mapping = build_type_mapping(&model.namespace, &schema);

        if !type_mapping.is_empty() {
            // add models objects & unions
            let field_name = utils::field_name_from_names(&model.namespace, &model.name);

            let type_name = utils::type_name_from_names(&model.namespace, &model.name);

            model_union = model_union.possible_type(&type_name);

            objects.push(ObjectVariant::Resolvable(Box::new(ModelDataObject::new(
                field_name,
                type_name,
                type_mapping.clone(),
                schema,
            ))));
        }
    }

    // When creating an empty union, add the empty type (this is required otherwise the schema will be invalid)
    if models.is_empty() {
        model_union = model_union.possible_type(EMPTY_TYPE_NAME);
    }

    unions.push(model_union);

    Ok((objects, unions))
}
