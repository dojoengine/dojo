// import { World, Manifest } from '../types';

// // Define Component class
// class Component {
//     entity: number;

//     constructor(entity: number) {
//         this.entity = entity;
//     }
// }

// // Define Entity class
// class Entity {
//     id: number;
//     components: Component[];

//     constructor(id: number) {
//         this.id = id;
//         this.components = [];
//     }

//     addComponent(component: Component) {
//         this.components.push(component);
//     }
// }

// // Define System class
// class System {
//     // entities: Entity[];

//     constructor() {
//         // this.entities = [];
//     }

//     // addEntity(entity: Entity) {
//     //     this.entities.push(entity);
//     // }

//     // update() {
//     //     // implementation depends on the specifics of the system
//     // }
// }

// // Define an ECS class to manage Entities, Components and Systems
// class ECS {
//     world: string;
//     entities: Entity[];
//     systems: System[];

//     constructor(manifest: Manifest) {
//         this.world = manifest.world;
//         this.entities = [];
//         this.systems = manifest.systems;
//     }

//     createEntity() {
//         const id = this.entities.length;
//         const entity = new Entity(id);
//         this.entities.push(entity);
//         return entity;
//     }

//     addSystem(system: System) {
//         this.systems.push(system);
//     }

//     update() {
//         this.systems.forEach(system => system.update());
//     }
// }