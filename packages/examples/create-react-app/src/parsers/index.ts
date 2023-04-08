import { Realm } from "../types";

export const RealmParser = (metadata: number[]): Realm => {
    return {
        id: metadata[0],
        name: `Realm ${metadata[1]}`,
        description: `This is Realm ${metadata[1]}`,
        owner: metadata[2],
        armies: metadata.slice(3),
    };
};