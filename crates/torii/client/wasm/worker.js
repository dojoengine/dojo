// The worker has its own scope and no direct access to functions/objects of the
// global scope. We import the generated JS file to make `wasm_bindgen`
// available which we need to initialize our Wasm code.
importScripts("./pkg/torii_client_wasm.js");

// In the worker, we have a different struct that we want to use as in
// `index.js`.
const { createClient } = wasm_bindgen;

async function setup() {
	console.log("Initializing torii client worker 🚧");

	// Load the wasm file by awaiting the Promise returned by `wasm_bindgen`.
	await wasm_bindgen("./pkg/torii_client_wasm_bg.wasm");

	try {
		const client = await createClient(
			[
				{
					model: "Position",
					keys: [
						"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
					],
				},
				{
					model: "Moves",
					keys: [
						"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
					],
				},
			],
			{
				rpcUrl: "http://0.0.0.0:5050",
				toriiUrl: "http://0.0.0.0:8080/grpc",
				worldAddress:
					"0x1af130f7b9027f3748c1e3b10ca4a82ac836a30ac4f2f84025e83a99a922a0c",
			}
		);

		client.onEntityChange(
			{
				model: "Position",
				keys: [
					"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
				],
			},
			() => {
				const values = client.getModelValue("Position", [
					"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
				]);
				console.log("Position changed", values);
			}
		);

		client.onEntityChange(
			{
				model: "Moves",
				keys: [
					"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
				],
			},
			() => {
				const values = client.getModelValue("Moves", [
					"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
				]);
				console.log("Moves changed", values);
			}
		);

		// setTimeout(() => {
		// 	client.addEntitiesToSync([
		// 		{
		// 			model: "Moves",
		// 			keys: [
		// 				"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
		// 			],
		// 		},
		// 	]);
		// }, 10000);

		// // setup the message handler for the worker
		// self.onmessage = function (e) {
		// 	const event = e.data.type;
		// 	const data = e.data.data;

		// 	if (event === "getModelValue") {
		// 		getModelValueHandler(client, data);
		// 	} else if (event === "addEntityToSync") {
		// 		addEntityToSyncHandler(client, data);
		// 	} else {
		// 		console.log("Sync Worker: Unknown event type", event);
		// 	}
		// };

		console.log("Torii client initialized 🔥");
	} catch (e) {
		console.error("Error initiating torii client: ", e);
	}
}

// function addEntityToSyncHandler(client, data) {
// 	console.log("Sync Worker | Adding new entity to sync | data: ", data);
// 	client.addEntityToSync(data);
// }

/// Handler for the `get_entity` event from the main thread.
/// Returns back the entity data to the main thread via `postMessage`.
async function getModelValueHandler(client, data) {
	const model = data.model;
	const keys = data.keys;

	const values = client.getModelValue(model, keys);

	console.log("Sync Worker | Got model value | values: ", values);

	// self.postMessage({
	// 	type: "getModelValue",
	// 	data: {
	// 		model: "Position",
	// 		keys,
	// 		values,
	// 	},
	// });
}

setup();
