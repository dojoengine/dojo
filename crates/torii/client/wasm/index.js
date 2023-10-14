async function run_wasm() {
	// Load the wasm file by awaiting the Promise returned by `wasm_bindgen`
	// `wasm_bindgen` was imported in `index.html`
	await wasm_bindgen();

	console.log("index.js loaded");

	const clientWorker = new Worker("./worker.js");

	clientWorker.onmessage = function (e) {
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
		setInterval(() => {
			// Get the entity values from the sync worker
			clientWorker.postMessage({
				type: "getModelValue",
				data: {
					model: "Position",
					keys: [
						"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
					],
				},
			});

			// Get the entity values from the sync worker
			clientWorker.postMessage({
				type: "getModelValue",
				data: {
					model: "Moves",
					keys: [
						"0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
					],
				},
			});
		}, 2000);
	}, 1000);
}

run_wasm();
