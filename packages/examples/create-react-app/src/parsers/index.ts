import { Position } from "../types";

interface Data {
    entity: number[];
}

export const PositionParser = (data: Data): Position => {
    return {
        x: data.entity[0],
        y: data.entity[1],
    };
};