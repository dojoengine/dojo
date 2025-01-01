#[async_trait]
impl DatabaseBackend for PostgresBackend {
    type Pool = Pool<Postgres>;
    type Connection = sqlx::PgConnection;

    async fn new(
        pool: Self::Pool,
        executor: UnboundedSender<QueryMessage>,
        contracts: &[Contract],
        model_cache: Arc<ModelCache>,
    ) -> Result<Self> {
        for contract in contracts {
            executor.send(QueryMessage::other(
                "INSERT INTO contracts (id, contract_address, contract_type) VALUES ($1, $2, $3) \
                 ON CONFLICT (id) DO NOTHING"
                    .to_string(),
                vec![
                    Argument::FieldElement(contract.address),
                    Argument::FieldElement(contract.address),
                    Argument::String(contract.r#type.to_string()),
                ],
            ))?;
        }

        let local_cache = LocalCache::new(pool.clone()).await;
        let db = Self { pool: pool.clone(), executor, model_cache, local_cache };
        db.execute().await?;
        Ok(db)
    }

    async fn head(&self, contract: Felt) -> Result<(u64, Option<Felt>, Option<Felt>)> {
        let indexer_query = sqlx::query_as::<
            _,
            (Option<i64>, Option<String>, Option<String>, String),
        >(
            "SELECT head, last_pending_block_contract_tx, last_pending_block_tx, contract_type \
             FROM contracts WHERE id = $1",
        )
        .bind(format!("{:#x}", contract));

        let indexer: (Option<i64>, Option<String>, Option<String>, String) = indexer_query
            .fetch_one(&self.pool)
            .await
            .with_context(|| format!("Failed to fetch head for contract: {:#x}", contract))?;

        Ok((
            indexer
                .0
                .map(|h| h.try_into().map_err(|_| anyhow!("Head value {} doesn't fit in u64", h)))
                .transpose()?
                .unwrap_or(0),
            indexer.1.map(|f| Felt::from_str(&f)).transpose()?,
            indexer.2.map(|f| Felt::from_str(&f)).transpose()?,
        ))
    }

    async fn set_head(
        &mut self,
        head: u64,
        last_block_timestamp: u64,
        world_txns_count: u64,
        contract_address: Felt,
    ) -> Result<()> {
        let head_arg = Argument::Int(
            head.try_into().map_err(|_| anyhow!("Head value {} doesn't fit in i64", head))?,
        );
        let last_block_timestamp_arg =
            Argument::Int(last_block_timestamp.try_into().map_err(|_| {
                anyhow!("Last block timestamp value {} doesn't fit in i64", last_block_timestamp)
            })?);
        let id = Argument::FieldElement(contract_address);

        self.executor.send(QueryMessage::new(
            "UPDATE contracts SET head = $1, last_block_timestamp = $2 WHERE id = $3".to_string(),
            vec![head_arg, last_block_timestamp_arg, id],
            QueryType::SetHead(SetHeadQuery {
                head,
                last_block_timestamp,
                txns_count: world_txns_count,
                contract_address,
            }),
        ))?;

        Ok(())
    }

    fn set_last_pending_block_contract_tx(
        &mut self,
        contract: Felt,
        last_pending_block_contract_tx: Option<Felt>,
    ) -> Result<()> {
        let last_pending_block_contract_tx = if let Some(f) = last_pending_block_contract_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        let id = Argument::FieldElement(contract);

        self.executor.send(QueryMessage::other(
            "UPDATE contracts SET last_pending_block_contract_tx = $1 WHERE id = $2".to_string(),
            vec![last_pending_block_contract_tx, id],
        ))?;

        Ok(())
    }

    fn set_last_pending_block_tx(&mut self, last_pending_block_tx: Option<Felt>) -> Result<()> {
        let last_pending_block_tx = if let Some(f) = last_pending_block_tx {
            Argument::String(format!("{:#x}", f))
        } else {
            Argument::Null
        };

        self.executor.send(QueryMessage::other(
            "UPDATE contracts SET last_pending_block_tx = $1 WHERE true".to_string(),
            vec![last_pending_block_tx],
        ))?;

        Ok(())
    }

    async fn cursors(&self) -> Result<Cursors> {
        let mut conn = self.pool.acquire().await?;
        let cursors = sqlx::query_as::<_, (String, String)>(
            "SELECT contract_address, last_pending_block_contract_tx FROM contracts WHERE \
             last_pending_block_contract_tx IS NOT NULL",
        )
        .fetch_all(&mut *conn)
        .await?;

        let (head, last_pending_block_tx) = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
            "SELECT head, last_pending_block_tx FROM contracts WHERE true",
        )
        .fetch_one(&mut *conn)
        .await?;

        let head = head.map(|h| h.try_into().expect("doesn't fit in u64"));
        let last_pending_block_tx =
            last_pending_block_tx.map(|t| Felt::from_str(&t).expect("its a valid felt"));

        Ok(Cursors {
            cursor_map: cursors
                .into_iter()
                .map(|(c, t)| {
                    (
                        Felt::from_str(&c).expect("its a valid felt"),
                        Felt::from_str(&t).expect("its a valid felt"),
                    )
                })
                .collect(),
            last_pending_block_tx,
            head,
        })
    }

    fn reset_cursors(
        &mut self,
        head: u64,
        cursor_map: HashMap<Felt, (Felt, u64)>,
        last_block_timestamp: u64,
    ) -> Result<()> {
        self.executor.send(QueryMessage::new(
            "".to_string(),
            vec![],
            QueryType::ResetCursors(ResetCursorsQuery {
                cursor_map,
                last_block_timestamp,
                last_block_number: head,
            }),
        ))?;

        Ok(())
    }

    fn update_cursors(
        &mut self,
        head: u64,
        last_pending_block_tx: Option<Felt>,
        cursor_map: HashMap<Felt, (Felt, u64)>,
        pending_block_timestamp: u64,
    ) -> Result<()> {
        self.executor.send(QueryMessage::new(
            "".to_string(),
            vec![],
            QueryType::UpdateCursors(UpdateCursorsQuery {
                cursor_map,
                last_pending_block_tx,
                last_block_number: head,
                pending_block_timestamp,
            }),
        ))?;
        Ok(())
    }

    async fn register_model(
        &mut self,
        namespace: &str,
        model: &Ty,
        layout: Layout,
        class_hash: Felt,
        contract_address: Felt,
        packed_size: u32,
        unpacked_size: u32,
        block_timestamp: u64,
        upgrade_diff: Option<&Ty>,
    ) -> Result<()> {
        let selector = compute_selector_from_names(namespace, &model.name());
        let namespaced_name = get_tag(namespace, &model.name());
        let namespaced_schema = Ty::Struct(Struct {
            name: namespaced_name.clone(),
            children: model.as_struct().unwrap().children.clone(),
        });

        let insert_models =
            "INSERT INTO models (id, namespace, name, class_hash, contract_address, layout, \
             schema, packed_size, unpacked_size, executed_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT(id) DO UPDATE SET \
             contract_address = EXCLUDED.contract_address, \
             class_hash = EXCLUDED.class_hash, \
             layout = EXCLUDED.layout, \
             schema = EXCLUDED.schema, \
             packed_size = EXCLUDED.packed_size, \
             unpacked_size = EXCLUDED.unpacked_size, \
             executed_at = EXCLUDED.executed_at \
             RETURNING *";

        let arguments = vec![
            Argument::String(format!("{:#x}", selector)),
            Argument::String(namespace.to_string()),
            Argument::String(model.name().to_string()),
            Argument::String(format!("{class_hash:#x}")),
            Argument::String(format!("{contract_address:#x}")),
            Argument::String(serde_json::to_string(&layout)?),
            Argument::String(serde_json::to_string(&namespaced_schema)?),
            Argument::Int(packed_size as i64),
            Argument::Int(unpacked_size as i64),
            Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
        ];

        self.executor.send(QueryMessage::new(
            insert_models.to_string(),
            arguments,
            QueryType::RegisterModel,
        ))?;

        self.build_model_query(vec![namespaced_name.clone()], model, upgrade_diff)?;

        self.model_cache
            .set(
                selector,
                Model {
                    namespace: namespace.to_string(),
                    name: model.name().to_string(),
                    selector,
                    class_hash,
                    contract_address,
                    packed_size,
                    unpacked_size,
                    layout,
                    schema: namespaced_schema,
                },
            )
            .await;

        Ok(())
    }

    async fn set_entity(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
        entity_id: Felt,
        model_id: Felt,
        keys_str: Option<&str>,
    ) -> Result<()> {
        let namespaced_name = entity.name();
        let entity_id = format!("{:#x}", entity_id);
        let model_id = format!("{:#x}", model_id);

        let insert_entities = if keys_str.is_some() {
            "INSERT INTO entities (id, event_id, executed_at, keys) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT(id) DO UPDATE SET \
             updated_at = CURRENT_TIMESTAMP, \
             executed_at = EXCLUDED.executed_at, \
             event_id = EXCLUDED.event_id, \
             keys = EXCLUDED.keys \
             RETURNING *"
        } else {
            "INSERT INTO entities (id, event_id, executed_at) \
             VALUES ($1, $2, $3) \
             ON CONFLICT(id) DO UPDATE SET \
             updated_at = CURRENT_TIMESTAMP, \
             executed_at = EXCLUDED.executed_at, \
             event_id = EXCLUDED.event_id \
             RETURNING *"
        };

        let mut arguments = vec![
            Argument::String(entity_id.clone()),
            Argument::String(event_id.to_string()),
            Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
        ];

        if let Some(keys) = keys_str {
            arguments.push(Argument::String(keys.to_string()));
        }

        self.executor.send(QueryMessage::new(
            insert_entities.to_string(),
            arguments,
            QueryType::SetEntity(entity.clone()),
        ))?;

        self.executor.send(QueryMessage::other(
            "INSERT INTO entity_model (entity_id, model_id) \
             VALUES ($1, $2) \
             ON CONFLICT(entity_id, model_id) DO NOTHING"
                .to_string(),
            vec![Argument::String(entity_id.clone()), Argument::String(model_id.clone())],
        ))?;

        self.set_entity_model(
            &namespaced_name,
            event_id,
            (&entity_id, false),
            (&entity, keys_str.is_none()),
            block_timestamp,
        )?;

        Ok(())
    }

    async fn set_event_message(
        &mut self,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
        is_historical: bool,
    ) -> Result<()> {
        let keys = if let Ty::Struct(s) = &entity {
            let mut keys = Vec::new();
            for m in s.keys() {
                keys.extend(m.serialize()?);
            }
            keys
        } else {
            return Err(anyhow!("Entity is not a struct"));
        };

        let namespaced_name = entity.name();
        let (model_namespace, model_name) = namespaced_name.split_once('-').unwrap();

        let entity_id = format!("{:#x}", poseidon_hash_many(&keys));
        let model_id = format!("{:#x}", compute_selector_from_names(model_namespace, model_name));

        let keys_str = felts_to_sql_string(&keys);
        let block_timestamp_str = utc_dt_string_from_timestamp(block_timestamp);

        let insert_entities = "INSERT INTO event_messages (id, keys, event_id, executed_at) \
                             VALUES ($1, $2, $3, $4) \
                             ON CONFLICT(id) DO UPDATE SET \
                             updated_at = CURRENT_TIMESTAMP, \
                             executed_at = EXCLUDED.executed_at, \
                             event_id = EXCLUDED.event_id \
                             RETURNING *";

        self.executor.send(QueryMessage::new(
            insert_entities.to_string(),
            vec![
                Argument::String(entity_id.clone()),
                Argument::String(keys_str.clone()),
                Argument::String(event_id.to_string()),
                Argument::String(block_timestamp_str.clone()),
            ],
            QueryType::EventMessage(EventMessageQuery {
                entity_id: entity_id.clone(),
                model_id: model_id.clone(),
                keys_str: keys_str.clone(),
                event_id: event_id.to_string(),
                block_timestamp: block_timestamp_str.clone(),
                ty: entity.clone(),
                is_historical,
            }),
        ))?;

        self.set_entity_model(
            &namespaced_name,
            event_id,
            (&entity_id, true),
            (&entity, false),
            block_timestamp,
        )?;

        Ok(())
    }

    async fn delete_entity(
        &mut self,
        entity_id: Felt,
        model_id: Felt,
        entity: Ty,
        event_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
        let entity_id = format!("{:#x}", entity_id);
        let model_table = entity.name();

        self.executor.send(QueryMessage::new(
            format!(
                "DELETE FROM \"{model_table}\" WHERE internal_id = $1; \
                 DELETE FROM entity_model WHERE entity_id = $1 AND model_id = $2"
            ),
            vec![Argument::String(entity_id.clone()), Argument::String(format!("{:#x}", model_id))],
            QueryType::DeleteEntity(DeleteEntityQuery {
                entity_id: entity_id.clone(),
                event_id: event_id.to_string(),
                block_timestamp: utc_dt_string_from_timestamp(block_timestamp),
                ty: entity.clone(),
            }),
        ))?;

        Ok(())
    }

    fn set_metadata(&mut self, resource: &Felt, uri: &str, block_timestamp: u64) -> Result<()> {
        let resource = Argument::FieldElement(*resource);
        let uri = Argument::String(uri.to_string());
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.executor.send(QueryMessage::other(
            "INSERT INTO metadata (id, uri, executed_at) \
             VALUES ($1, $2, $3) \
             ON CONFLICT(id) DO UPDATE SET \
             id = EXCLUDED.id, \
             executed_at = EXCLUDED.executed_at, \
             updated_at = CURRENT_TIMESTAMP"
                .to_string(),
            vec![resource, uri, executed_at],
        ))?;

        Ok(())
    }

    fn update_metadata(
        &mut self,
        resource: &Felt,
        uri: &str,
        metadata: &WorldMetadata,
        icon_img: &Option<String>,
        cover_img: &Option<String>,
    ) -> Result<()> {
        let json = serde_json::to_string(metadata).unwrap(); // safe unwrap

        let mut update = vec!["uri = $1", "json = $2", "updated_at = CURRENT_TIMESTAMP"];
        let mut arguments = vec![Argument::String(uri.to_string()), Argument::String(json)];
        let mut param_count = 2;

        if let Some(icon) = icon_img {
            param_count += 1;
            update.push(&format!("icon_img = ${param_count}"));
            arguments.push(Argument::String(icon.clone()));
        }

        if let Some(cover) = cover_img {
            param_count += 1;
            update.push(&format!("cover_img = ${param_count}"));
            arguments.push(Argument::String(cover.clone()));
        }

        param_count += 1;
        let statement =
            format!("UPDATE metadata SET {} WHERE id = ${}", update.join(","), param_count);
        arguments.push(Argument::FieldElement(*resource));

        self.executor.send(QueryMessage::other(statement, arguments))?;

        Ok(())
    }

    async fn model(&self, selector: Felt) -> Result<Model> {
        self.model_cache.model(&selector).await.map_err(|e| e.into())
    }

    async fn does_entity_exist(&self, model: String, key: Felt) -> Result<bool> {
        let sql = format!("SELECT COUNT(*) FROM \"{}\" WHERE id = $1", model);

        let count: i64 =
            sqlx::query_scalar(&sql).bind(format!("{:#x}", key)).fetch_one(&self.pool).await?;

        Ok(count > 0)
    }

    async fn entities(&self, model: String) -> Result<Vec<Vec<Felt>>> {
        let query = sqlx::query_as::<_, (i32, String, String)>("SELECT * FROM $1").bind(model);
        let mut conn = self.pool.acquire().await?;
        let mut rows = query.fetch_all(&mut *conn).await?;
        Ok(rows.drain(..).map(|row| serde_json::from_str(&row.2).unwrap()).collect())
    }

    fn store_transaction(
        &mut self,
        transaction: &Transaction,
        transaction_id: &str,
        block_timestamp: u64,
    ) -> Result<()> {
        let id = Argument::String(transaction_id.to_string());

        let transaction_type = match transaction {
            Transaction::Invoke(_) => "INVOKE",
            Transaction::L1Handler(_) => "L1_HANDLER",
            _ => return Ok(()),
        };

        let (transaction_hash, sender_address, calldata, max_fee, signature, nonce) =
            match transaction {
                Transaction::Invoke(InvokeTransaction::V3(invoke_v3_transaction)) => (
                    Argument::FieldElement(invoke_v3_transaction.transaction_hash),
                    Argument::FieldElement(invoke_v3_transaction.sender_address),
                    Argument::String(felts_to_sql_string(&invoke_v3_transaction.calldata)),
                    Argument::FieldElement(Felt::ZERO), // has no max_fee
                    Argument::String(felts_to_sql_string(&invoke_v3_transaction.signature)),
                    Argument::FieldElement(invoke_v3_transaction.nonce),
                ),
                Transaction::Invoke(InvokeTransaction::V1(invoke_v1_transaction)) => (
                    Argument::FieldElement(invoke_v1_transaction.transaction_hash),
                    Argument::FieldElement(invoke_v1_transaction.sender_address),
                    Argument::String(felts_to_sql_string(&invoke_v1_transaction.calldata)),
                    Argument::FieldElement(invoke_v1_transaction.max_fee),
                    Argument::String(felts_to_sql_string(&invoke_v1_transaction.signature)),
                    Argument::FieldElement(invoke_v1_transaction.nonce),
                ),
                Transaction::L1Handler(l1_handler_transaction) => (
                    Argument::FieldElement(l1_handler_transaction.transaction_hash),
                    Argument::FieldElement(l1_handler_transaction.contract_address),
                    Argument::String(felts_to_sql_string(&l1_handler_transaction.calldata)),
                    Argument::FieldElement(Felt::ZERO), // has no max_fee
                    Argument::String("".to_string()),   // has no signature
                    Argument::FieldElement((l1_handler_transaction.nonce).into()),
                ),
                _ => return Ok(()),
            };

        self.executor.send(QueryMessage::other(
            "INSERT INTO transactions \
             (id, transaction_hash, sender_address, calldata, max_fee, signature, nonce, \
             transaction_type, executed_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (id) DO NOTHING"
                .to_string(),
            vec![
                id,
                transaction_hash,
                sender_address,
                calldata,
                max_fee,
                signature,
                nonce,
                Argument::String(transaction_type.to_string()),
                Argument::String(utc_dt_string_from_timestamp(block_timestamp)),
            ],
        ))?;

        Ok(())
    }

    fn store_event(
        &mut self,
        event_id: &str,
        event: &Event,
        transaction_hash: Felt,
        block_timestamp: u64,
    ) -> Result<()> {
        let id = Argument::String(event_id.to_string());
        let keys = Argument::String(felts_to_sql_string(&event.keys));
        let data = Argument::String(felts_to_sql_string(&event.data));
        let hash = Argument::FieldElement(transaction_hash);
        let executed_at = Argument::String(utc_dt_string_from_timestamp(block_timestamp));

        self.executor.send(QueryMessage::new(
            "INSERT INTO events (id, keys, data, transaction_hash, executed_at) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (id) DO NOTHING \
             RETURNING *"
                .to_string(),
            vec![id, keys, data, hash, executed_at],
            QueryType::StoreEvent,
        ))?;

        Ok(())
    }

    async fn execute(&self) -> Result<()> {
        let (execute, recv) = QueryMessage::execute_recv();
        self.executor.send(execute)?;
        recv.await?
    }

    async fn flush(&self) -> Result<()> {
        let (flush, recv) = QueryMessage::flush_recv();
        self.executor.send(flush)?;
        recv.await?
    }

    async fn rollback(&self) -> Result<()> {
        let (rollback, recv) = QueryMessage::rollback_recv();
        self.executor.send(rollback)?;
        recv.await?
    }
}
