declare class RPCError extends Error {
    code: number;
    data?: any;
    constructor(message: string, code: number, data?: any);
}
export { RPCError };
