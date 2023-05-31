import Store from '../dist/store/index.js';
import Manifest from './manifest.json' assert { type: 'json' };

const main = async () => {

    Store.registerWorld(Manifest);

    console.log(Store.getWorld());

    const position = Store.getComponent('Position');

    console.log(position);

};

main();