"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RPCError = void 0;
class RPCError extends Error {
    constructor(message, code, data) {
        super(message);
        this.name = "RPCError";
        this.code = code;
        this.data = data;
        Object.setPrototypeOf(this, new.target.prototype);
    }
}
exports.RPCError = RPCError;
