import { RPCProvider } from "../rpcProvider";
import { RPCError } from "../error/rpcError";
import fetchMock from "fetch-mock";

describe("RPCProvider", () => {
    const rpcUrl = "http://localhost:8080";
    const provider = new RPCProvider(rpcUrl);

    afterEach(() => {
        fetchMock.reset();
    });

    it("should perform a successful JSON-RPC call", async () => {
        const method = "getGreeting";
        const expectedResult = "Hello, RPC!";

        fetchMock.post(rpcUrl, {
            jsonrpc: "2.0",
            id: 1,
            result: expectedResult,
        });

        const result = await provider.call(method);

        expect(result).toBe(expectedResult);
    });

    it("should handle JSON-RPC errors", async () => {
        const method = "getGreeting";
        const errorCode = -32000;
        const errorMessage = "RPC Error";

        fetchMock.post(rpcUrl, {
            jsonrpc: "2.0",
            id: 1,
            error: {
                code: errorCode,
                message: errorMessage,
            },
        });

        try {
            await provider.call(method);
        } catch (error) {
            expect(error).toBeInstanceOf(Error);
            expect((error as Error).message).toBe(errorMessage);
            expect((error as RPCError).code).toBe(errorCode);
        }
    });
});