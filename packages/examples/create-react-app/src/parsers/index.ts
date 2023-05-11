import { Position } from "../types";

export const PositionParser = (data: number[]): Position => {

    console.log(data)
    return {
        x: data[0],
        y: data[1],
    };
};