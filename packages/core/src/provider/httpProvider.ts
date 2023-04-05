import { Provider } from "./provider";
import { IRequestOptions } from "./types";

// httpProvider.ts
export class HttpProvider extends Provider {
    public async send(request: IRequestOptions): Promise<any> {
        try {
            const response = await fetch(request.url, {
                method: request.method,
                body: JSON.stringify(request.data),
                headers: request.headers,
            });

            const data = await response.json();
            this.emit("response", data);
            return data;
        } catch (error) {
            this.emit("error", error);
            throw error;
        }
    }
}