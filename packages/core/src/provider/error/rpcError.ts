class RPCError extends Error {
    public code: number;
    public data?: any;

    constructor(message: string, code: number, data?: any) {
        super(message);
        this.name = "RPCError";
        this.code = code;
        this.data = data;
        Object.setPrototypeOf(this, new.target.prototype);
    }
}

export { RPCError };