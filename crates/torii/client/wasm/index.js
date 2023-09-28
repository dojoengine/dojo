// We only need `startup` here which is the main entry point
// In theory, we could also use all other functions/struct types from Rust which we have bound with
// `#[wasm_bindgen]`
const { setup } = wasm_bindgen;

async function run_wasm() {
	// Load the wasm file by awaiting the Promise returned by `wasm_bindgen`
	// `wasm_bindgen` was imported in `index.html`
	await wasm_bindgen();

	console.log("index.js loaded");

	const syncWorker = new Worker("./worker.js");

	syncWorker.onmessage = function (e) {
		const event = e.data.type;
		const data = e.data.data;

		if (event === "getModelValue") {
			console.log(
				"Main thread | model: ",
				data.model,
				"keys: ",
				data.keys,
				"values: ",
				data.values
			);
		} else {
			console.log("Sync Worker: Unknown event type", event);
		}
	};

	setTimeout(() => {
		// Add the entity to sync
		syncWorker.postMessage({
			type: "addEntityToSync",
			data: {
				model: "Position",
				keys: [
					"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
				],
			},
		});

		setInterval(() => {
			// Get the entity values from the sync worker
			syncWorker.postMessage({
				type: "getModelValue",
				data: {
					model: "Position",
					keys: [
						"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
					],
					length: 2,
				},
			});
		}, 1000);
	}, 1000);
}

run_wasm();
