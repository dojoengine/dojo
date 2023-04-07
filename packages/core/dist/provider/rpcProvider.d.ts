import { HttpProvider } from "./httpProvider";
export declare class RPCProvider extends HttpProvider {
    private url;
    private requestId;
    constructor(url: string);
    call(method: string, params?: any[]): Promise<any>;
}
