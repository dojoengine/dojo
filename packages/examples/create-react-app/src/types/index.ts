export interface QueryResult<T> {
    data: T | undefined;
    loading: boolean;
    error: any;
}

export interface Component {
    key: number;
}

export interface Position {
    x: number;
    y: number;
}

export interface Moves {
    remaining: number;
}