import { useState, useEffect, useCallback } from 'react';
import { Position } from './Position';

type Entity = {
    id: string;
    src: string;
    position: {
        x: number;
        y: number;
    };
    direction: number;
};


// replace with actual indexer fetch
const entities = [
    { id: 'A', src: 'player.png', position: { x: 10, y: 10 }, direction: -1 },
    { id: 'B', src: 'player.png', position: { x: 20, y: 20 }, direction: -1 },
    { id: 'C', src: 'player.png', position: { x: 30, y: 30 }, direction: -1 },
    { id: 'D', src: 'nazi.png', position: { x: 2, y: 3 }, direction: -1 },
    { id: 'E', src: 'nazi.png', position: { x: 12, y: 33 }, direction: -1 },
    { id: 'F', src: 'nazi.png', position: { x: 4, y: 34 }, direction: -1 },
    { id: 'G', src: 'player.png', position: { x: 10, y: 10 }, direction: -1 },
    { id: 'H', src: 'player.png', position: { x: 20, y: 20 }, direction: -1 },
    { id: 'I', src: 'player.png', position: { x: 30, y: 30 }, direction: -1 },
    { id: 'G', src: 'nazi.png', position: { x: 2, y: 3 }, direction: -1 },
    { id: 'K', src: 'nazi.png', position: { x: 12, y: 33 }, direction: -1 },
    { id: 'L', src: 'nazi.png', position: { x: 4, y: 34 }, direction: -1 },
    { id: 'M', src: 'player.png', position: { x: 10, y: 10 }, direction: -1 },
    { id: 'N', src: 'player.png', position: { x: 20, y: 20 }, direction: -1 },
    { id: 'O', src: 'player.png', position: { x: 30, y: 30 }, direction: -1 },
    { id: 'P', src: 'nazi.png', position: { x: 2, y: 3 }, direction: -1 },
    { id: 'Q', src: 'nazi.png', position: { x: 12, y: 33 }, direction: -1 },
    { id: 'R', src: 'nazi.png', position: { x: 4, y: 34 }, direction: -1 },
    { id: 'S', src: 'player.png', position: { x: 10, y: 10 }, direction: -1 },
    { id: 'W', src: 'player.png', position: { x: 20, y: 20 }, direction: -1 }
];

const GridComponent = () => {
    const gridSize = 50;
    const [selectedEntity, setSelectedEntity] = useState<Entity | null>(null);

    const handleCellClick = (entity: Entity | null) => {
        setSelectedEntity(entity);
    };

    const handleKeyDown = useCallback(
        (event: KeyboardEvent) => {
            if (!selectedEntity) return;

            const oldPosition = { ...selectedEntity.position };
            const newPosition = { ...selectedEntity.position };
            switch (event.key) {
                case 'ArrowUp':
                    newPosition.y -= 1;
                    break;
                case 'ArrowDown':
                    newPosition.y += 1;
                    break;
                case 'ArrowLeft':
                    newPosition.x -= 1;
                    break;
                case 'ArrowRight':
                    newPosition.x += 1;
                    break;
                default:
                    return;
            }

            if (newPosition.x >= 0 && newPosition.x < gridSize && newPosition.y >= 0 && newPosition.y < gridSize) {
                const direction = getDirection([oldPosition.x, oldPosition.y], [newPosition.x, newPosition.y]);
                setSelectedEntity({ ...selectedEntity, position: { ...newPosition }, direction });
            }
        },
        [selectedEntity, gridSize]
    );

    useEffect(() => {
        document.addEventListener('keydown', handleKeyDown);
        return () => {
            document.removeEventListener('keydown', handleKeyDown);
        };
    }, [handleKeyDown]);

    const grid: (Entity | null)[][] = Array(gridSize)
        .fill(null)
        .map(() => Array(gridSize).fill(null));

    entities
        .filter((entity) => !selectedEntity || entity.id !== selectedEntity.id)
        .concat(selectedEntity ? [selectedEntity] : [])
        .forEach((entity) => {
            if (!entity) return;
            const { x, y } = entity.position;
            if (x >= 0 && x < gridSize && y >= 0 && y < gridSize) {
                grid[y][x] = entity;
            }
        });

    return (
        <div className="grid-container" style={{ display: 'grid', gridTemplateColumns: `repeat(${gridSize}, 1fr)`, border: '1px solid black' }}>
            {grid.map((row, rowIndex) =>
                row.map((cell, colIndex) => (
                    <div
                        key={`${rowIndex}-${colIndex}`}
                        className="grid-cell"
                        style={{ width: '20px', height: '20px', display: 'flex', justifyContent: 'center', alignItems: 'center' }}
                        onClick={() => handleCellClick(cell)}
                    >
                        {cell ? <Position entityId={cell.id} src={cell.src} position={cell.position} direction={cell.direction} /> : null}
                    </div>
                ))
            )}
        </div>
    );
};

export default GridComponent;

function getDirection(currentPosition: any, newPosition: any) {
    const dx = newPosition[0] - currentPosition[0];
    const dy = newPosition[1] - currentPosition[1];

    if (dy === -1 && dx === 0) {
        return 0; // Up
    } else if (dy === 0 && dx === 1) {
        return 1; // Right
    } else if (dy === 1 && dx === 0) {
        return 2; // Down
    } else if (dy === 0 && dx === -1) {
        return 3; // Left
    } else {
        return -1; // Invalid move or no move
    }
}

