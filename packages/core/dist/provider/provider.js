"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Provider = void 0;
const events_1 = require("events");
class Provider extends events_1.EventEmitter {
    constructor(worldAddress) {
        super();
        this.worldAddress = worldAddress;
    }
    getWorldAddress() {
        return this.worldAddress;
    }
}
exports.Provider = Provider;
