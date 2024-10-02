# This script compares data across 'events', 'entities', 'transactions', 'balances', 'tokens', and 'erc_transfers' tables between two SQLite databases.
# Helpful to make sure any changes made in torii doesn't affect the resulting data.

import sqlite3
import argparse

def fetch_table_data(db_path, table_name, columns):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute(f"SELECT {', '.join(columns)} FROM {table_name}")
    data = cursor.fetchall()
    conn.close()
    return {row[0]: row[1:] for row in data}

def get_table_row_count(db_path, table_name):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute(f"SELECT COUNT(*) FROM {table_name}")
    count = cursor.fetchone()[0]
    conn.close()
    return count

def compare_data(data1, data2, table_name):
    differences_found = False
    for id, values in data1.items():
        if id in data2:
            if values != data2[id]:
                print(f"Mismatch found in {table_name} for ID {id}:")
                print(f"  Database 1: {values}")
                print(f"  Database 2: {data2[id]}")
                differences_found = True
        else:
            print(f"ID {id} found in {table_name} of Database 1 but not in Database 2")
            differences_found = True

    for id in data2:
        if id not in data1:
            print(f"ID {id} found in {table_name} of Database 2 but not in Database 1")
            differences_found = True

    if not differences_found:
        print(f"No differences found in {table_name}")

def compare_databases(db_path1, db_path2):
    # Columns to compare, ignoring time-dependent and event_id columns
    events_columns = ["id", "keys", "data", "transaction_hash"]
    entities_columns = ["id", "keys"]
    transactions_columns = ["id", "transaction_hash", "sender_address", "calldata", "max_fee", "signature", "nonce", "transaction_type"]
    balances_columns = ["id", "balance", "account_address", "contract_address", "token_id"]
    tokens_columns = ["id", "contract_address", "name", "symbol", "decimals"]
    erc_transfers_columns = ["id", "contract_address", "from_address", "to_address", "amount", "token_id"]

    # Fetch data from both databases
    events_data_db1 = fetch_table_data(db_path1, "events", events_columns)
    events_data_db2 = fetch_table_data(db_path2, "events", events_columns)
    entities_data_db1 = fetch_table_data(db_path1, "entities", entities_columns)
    entities_data_db2 = fetch_table_data(db_path2, "entities", entities_columns)
    transactions_data_db1 = fetch_table_data(db_path1, "transactions", transactions_columns)
    transactions_data_db2 = fetch_table_data(db_path2, "transactions", transactions_columns)
    balances_data_db1 = fetch_table_data(db_path1, "balances", balances_columns)
    balances_data_db2 = fetch_table_data(db_path2, "balances", balances_columns)
    tokens_data_db1 = fetch_table_data(db_path1, "tokens", tokens_columns)
    tokens_data_db2 = fetch_table_data(db_path2, "tokens", tokens_columns)
    erc_transfers_data_db1 = fetch_table_data(db_path1, "erc_transfers", erc_transfers_columns)
    erc_transfers_data_db2 = fetch_table_data(db_path2, "erc_transfers", erc_transfers_columns)

    # Get row counts from both databases
    events_count_db1 = get_table_row_count(db_path1, "events")
    events_count_db2 = get_table_row_count(db_path2, "events")
    entities_count_db1 = get_table_row_count(db_path1, "entities")
    entities_count_db2 = get_table_row_count(db_path2, "entities")
    transactions_count_db1 = get_table_row_count(db_path1, "transactions")
    transactions_count_db2 = get_table_row_count(db_path2, "transactions")
    balances_count_db1 = get_table_row_count(db_path1, "balances")
    balances_count_db2 = get_table_row_count(db_path2, "balances")
    tokens_count_db1 = get_table_row_count(db_path1, "tokens")
    tokens_count_db2 = get_table_row_count(db_path2, "tokens")
    erc_transfers_count_db1 = get_table_row_count(db_path1, "erc_transfers")
    erc_transfers_count_db2 = get_table_row_count(db_path2, "erc_transfers")

    # Print row counts
    print(f"Number of rows in events table: Database 1 = {events_count_db1}, Database 2 = {events_count_db2}")
    print(f"Number of rows in entities table: Database 1 = {entities_count_db1}, Database 2 = {entities_count_db2}")
    print(f"Number of rows in transactions table: Database 1 = {transactions_count_db1}, Database 2 = {transactions_count_db2}")
    print(f"Number of rows in balances table: Database 1 = {balances_count_db1}, Database 2 = {balances_count_db2}")
    print(f"Number of rows in tokens table: Database 1 = {tokens_count_db1}, Database 2 = {tokens_count_db2}")
    print(f"Number of rows in erc_transfers table: Database 1 = {erc_transfers_count_db1}, Database 2 = {erc_transfers_count_db2}")

    # Compare data
    print("\nComparing events table:")
    compare_data(events_data_db1, events_data_db2, "events")

    print("\nComparing entities table:")
    compare_data(entities_data_db1, entities_data_db2, "entities")

    print("\nComparing transactions table:")
    compare_data(transactions_data_db1, transactions_data_db2, "transactions")

    print("\nComparing balances table:")
    compare_data(balances_data_db1, balances_data_db2, "balances")

    print("\nComparing tokens table:")
    compare_data(tokens_data_db1, tokens_data_db2, "tokens")

    print("\nComparing erc_transfers table:")
    compare_data(erc_transfers_data_db1, erc_transfers_data_db2, "erc_transfers")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Compare data in events, entities, transactions, balances, tokens, and erc_transfers tables between two SQLite databases.")
    parser.add_argument("db_path1", help="Path to the first SQLite database")
    parser.add_argument("db_path2", help="Path to the second SQLite database")
    args = parser.parse_args()

    compare_databases(args.db_path1, args.db_path2)
