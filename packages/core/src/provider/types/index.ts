export interface IRequestOptions {
    method: string;
    url: string;
    data?: any;
    headers?: { [key: string]: string };
}