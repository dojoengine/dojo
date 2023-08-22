const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const webpack = require("webpack");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
	entry: "./index.js",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "index.js",
	},
	plugins: [
		new HtmlWebpackPlugin(),
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "."),
		}),
		// Have this example work in Edge which doesn't ship `TextEncoder` or
		// `TextDecoder` at this time.
		new webpack.ProvidePlugin({
			TextDecoder: ["text-encoding", "TextDecoder"],
			TextEncoder: ["text-encoding", "TextEncoder"],
		}),
	],

	// settings it to `development` seems to be getting this error https://github.com/rustwasm/wasm-pack/issues/981
	mode: "production",
	experiments: {
		asyncWebAssembly: true,
	},
};
