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

def table_exists(db_path, table_name):
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute(f"SELECT name FROM sqlite_master WHERE type='table' AND name='{table_name}'")
    exists = cursor.fetchone() is not None
    conn.close()
    return exists

def compare_databases(db_path1, db_path2):
    # Columns to compare, ignoring time-dependent and event_id columns
    table_columns = {
        "events": ["id", "keys", "data", "transaction_hash"],
        "entities": ["id", "keys"],
        "transactions": ["id", "transaction_hash", "sender_address", "calldata", "max_fee", "signature", "nonce", "transaction_type"],
        "balances": ["id", "balance", "account_address", "contract_address", "token_id"],
        "tokens": ["id", "contract_address", "name", "symbol", "decimals"],
        "erc_transfers": ["id", "contract_address", "from_address", "to_address", "amount", "token_id"]
    }

    for table_name, columns in table_columns.items():
        if table_exists(db_path1, table_name) and table_exists(db_path2, table_name):
            print(f"\nComparing {table_name} table:")
            
            # Fetch data from both databases
            data_db1 = fetch_table_data(db_path1, table_name, columns)
            data_db2 = fetch_table_data(db_path2, table_name, columns)

            # Get row counts from both databases
            count_db1 = get_table_row_count(db_path1, table_name)
            count_db2 = get_table_row_count(db_path2, table_name)

            # Print row counts
            print(f"Number of rows in {table_name} table: Database 1 = {count_db1}, Database 2 = {count_db2}")

            # Compare data
            compare_data(data_db1, data_db2, table_name)
        else:
            print(f"\nSkipping {table_name} table as it doesn't exist in one or both databases.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Compare data in events, entities, transactions, balances, tokens, and erc_transfers tables between two SQLite databases.")
    parser.add_argument("db_path1", help="Path to the first SQLite database")
    parser.add_argument("db_path2", help="Path to the second SQLite database")
    args = parser.parse_args()

    compare_databases(args.db_path1, args.db_path2)
