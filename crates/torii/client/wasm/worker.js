// The worker has its own scope and no direct access to functions/objects of the
// global scope. We import the generated JS file to make `wasm_bindgen`
// available which we need to initialize our Wasm code.
importScripts("./pkg/torii_client_wasm.js");

console.log("Initializing worker");

// In the worker, we have a different struct that we want to use as in
// `index.js`.
const { WasmClient } = wasm_bindgen;

async function setup() {
	// Load the wasm file by awaiting the Promise returned by `wasm_bindgen`.
	await wasm_bindgen("./pkg/torii_client_wasm_bg.wasm");

	const client = new WasmClient(
		"http://localhost:5050",
		"0x398c6b4f479e2a6181ae895ad34333b44e419e48098d2a9622f976216d044dd"
	);

	client.start();

	// setup the message handler for the worker
	self.onmessage = function (e) {
		const event = e.data.type;
		const data = e.data.data;

		if (event === "getModelValue") {
			getModelValueHandler(client, data);
		} else if (event === "addEntityToSync") {
			addEntityToSyncHandler(client, data);
		} else {
			console.log("Sync Worker: Unknown event type", event);
		}
	};
}

function addEntityToSyncHandler(client, data) {
	console.log("Sync Worker | Adding new entity to sync | data: ", data);
	client.addEntityToSync(data);
}

/// Handler for the `get_entity` event from the main thread.
/// Returns back the entity data to the main thread via `postMessage`.
async function getModelValueHandler(client, data) {
	console.log("Sync Worker | Getting model value | data: ", data);

	const model = data.model;
	const keys = data.keys;
	const length = data.length;

	const values = await client.getModelValue(model, keys, length);

	self.postMessage({
		type: "getModelValue",
		data: {
			model: "Position",
			keys,
			values,
		},
	});
}

setup();
