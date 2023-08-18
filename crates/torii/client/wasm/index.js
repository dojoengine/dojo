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

		if (event === "getComponentValue") {
			console.log(
				"Main thread | component: ",
				data.component,
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
				component: "Position",
				keys: [
					"0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0",
				],
			},
		});

		setInterval(() => {
			// Get the entity values from the sync worker
			syncWorker.postMessage({
				type: "getComponentValue",
				data: {
					component: "Position",
					keys: [
						"0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0",
					],
					length: 2,
				},
			});
		}, 1000);
	}, 1000);
}

run_wasm();
