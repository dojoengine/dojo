# Sozo MCP Server Instructions

This MCP server provides tools and resources for working with Dojo projects using Sozo.

## Tools

### Build Tool
Builds the project using the specified profile (defaults to `dev`).

**Usage:**
- Profile: Optional profile name (e.g., "dev", "release")

**Example:**
```json
{
  "profile": "dev"
}
```

### Test Tool
Runs tests for the project using the specified profile (defaults to `dev`).

**Usage:**
- Profile: Optional profile name (e.g., "dev", "release")

### Inspect Tool
Retrieves detailed information about the project's resources including models, contracts, events, and namespaces.

**Usage:**
- Profile: Optional profile name (e.g., "dev", "release")

**Important:** Use this tool to discover available contracts, their addresses, and namespaces before executing transactions.

### Migrate Tool
Migrates the project to the blockchain using the specified profile (defaults to `dev`). Migration is always differential, and Sozo will only migrate the changes to the project that need to be migrated.

The migration generates a Dojo manifest file which is at the root of the project and named `manifest_{profile}.json`. This file is used to track the state of the project and the contracts deployed to the blockchain.

**Usage:**
- Profile: Optional profile name (e.g., "dev", "release")

### Execute Tool
Executes transactions on the project using the specified profile (defaults to `dev`).

**Usage:**
- Profile: Optional profile name (e.g., "dev", "release")
- Contract: Contract identifier (can be contract address or contract tag like "namespace-name")
- Function Name: Name of the function to execute
- Calldata: Array of calldata values (see Calldata Format below)
- Manifest Path: Optional manifest path (uses server's manifest path if not provided)

**Contract Identifier:**
The contract parameter can be either:
- A contract address (hex format): `0x1234567890abcdef...`
- A contract tag (namespace-name format): `my_namespace-my_contract`

**Calldata Format:**
Space separated values e.g., `0x12345 128 u256:9999999999 str:'hello world'`.

Sozo supports prefixes for automatic type parsing:
- `u256:` - A 256-bit unsigned integer
- `sstr:` - A cairo short string (use quotes for spaces: `sstr:'hello world'`)
- `str:` - A cairo string (ByteArray) (use quotes for spaces: `str:'hello world'`)
- `int:` - A signed integer
- `arr:` - A dynamic array where each item fits on a single felt252
- `u256arr:` - A dynamic array of u256
- `farr:` - A fixed-size array where each item fits on a single felt252
- `u256farr:` - A fixed-size array of u256
- No prefix: A cairo felt or any type that fits into one felt

**Example:**
```json
{
  "profile": "dev",
  "contract": "my_namespace-my_contract",
  "function_name": "spawn",
  "calldata": ["u256:100", "str:'player_name'", "0x12345"]
}
```

## Resources

### Project Manifest
**URI:** `dojo://scarb/manifest`

Provides the Scarb project manifest (Scarb.toml) converted to JSON format. Contains project dependencies and configuration.

### Contract ABIs
**URI Template:** `dojo://contract/{profile}/{name}/abi`

Get the ABI for a specific contract in the given profile. Use this to understand the contract's interface and available functions.

**Example:** `dojo://contract/dev/my_contract/abi`

### Model ABIs
**URI Template:** `dojo://model/{profile}/{name}/abi`

Get the ABI for a specific model in the given profile. Models define the data structures in your Dojo world.

**Example:** `dojo://model/dev/player/abi`

### Event ABIs
**URI Template:** `dojo://event/{profile}/{name}/abi`

Get the ABI for a specific event in the given profile. Events are emitted by contracts and can be used for indexing.

**Example:** `dojo://event/dev/game_event/abi`

## Workflow Examples

### 1. Discovering Project Resources
1. Use the **inspect** tool to get an overview of all available resources
2. This will show you contracts, models, events, and their namespaces
3. Note the contract addresses and namespaces for later use
4. When contract are built, there is no namespace, only the contract name. The namespace is used to deploy the same contract multiple times with different instances in the same Dojo world.

### 2. Understanding Contract Interfaces
1. Use the **inspect** tool to find contract names and namespaces
2. Access contract ABIs using the resource template: `dojo://contract/{profile}/{name}/abi`
3. Review the ABI to understand available functions and their parameters

### 3. Executing Transactions
1. First, use **inspect** to discover available contracts and their addresses
2. Use contract ABI resources to understand the interface
3. Use the **execute** tool with either:
   - Contract address: `0x1234567890abcdef...`
   - Contract tag: `namespace-contract_name`
4. Provide calldata using the supported prefixes for automatic type parsing

### 4. Building and Testing
1. Use **build** tool to compile your project
2. Use **test** tool to run tests
3. Use **migrate** tool to deploy to the blockchain

## Important Notes

- **Namespaces:** While namespaces aren't directly useful for resource discovery, they are essential for executing transactions. Always include the namespace when calling contracts.
- **Profiles:** Different profiles (dev, release) may have different configurations and deployed addresses.
- **Contract Addresses:** Use the inspect tool to get the current contract addresses before executing transactions.
- **ABI Access:** Use the templated resources to get contract ABIs for understanding available functions.

## Error Handling

- If a resource is not found, check that the profile and resource name are correct
- Ensure the project is built before trying to access ABIs
