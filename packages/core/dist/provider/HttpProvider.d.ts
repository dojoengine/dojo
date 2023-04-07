import { Provider } from "./provider";
import { IRequestOptions } from "./types";
export declare class HttpProvider extends Provider {
    send(request: IRequestOptions): Promise<any>;
}
