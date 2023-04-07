"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.HttpProvider = void 0;
const provider_1 = require("./provider");
// httpProvider.ts
class HttpProvider extends provider_1.Provider {
    async send(request) {
        try {
            const response = await fetch(request.url, {
                method: request.method,
                body: JSON.stringify(request.data),
                headers: request.headers,
            });
            const data = await response.json();
            this.emit("response", data);
            return data;
        }
        catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
}
exports.HttpProvider = HttpProvider;
