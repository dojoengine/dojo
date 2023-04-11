import React, { FC, useState, useEffect, useCallback } from 'react';

type Entity = {
    id: string;
    src: string;
    position: {
        x: number;
        y: number;
    };
};

interface GridComponentProps {
    entities: Entity[];
}

const GridComponent: FC<GridComponentProps> = ({ entities }) => {
    const gridSize = 40;
    const [selectedEntity, setSelectedEntity] = useState<Entity | null>(null);

    const handleCellClick = (entity: Entity | null) => {
        setSelectedEntity(entity);
    };

    const handleKeyDown = useCallback(
        (event: KeyboardEvent) => {
            if (!selectedEntity) return;

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
                setSelectedEntity({ ...selectedEntity, position: newPosition });
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

    const filteredEntities = selectedEntity ? entities.filter(entity => entity.id !== selectedEntity.id) : entities;

    filteredEntities.concat(selectedEntity ? [selectedEntity] : []).forEach((entity) => {
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
                        style={{ border: '0.3px solid black', width: '20px', height: '20px', display: 'flex', justifyContent: 'center', alignItems: 'center' }}
                        onClick={() => handleCellClick(cell)}
                    >
                        {cell ? <img src={cell.src} alt={cell.id} style={{ width: '100%', height: '100%', objectFit: 'contain' }} /> : null}
                    </div>
                ))
            )}
        </div>
    );
};

export default GridComponent;