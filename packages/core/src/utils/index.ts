/**
 * Converts a string to a `felt252` representation.
 *
 * @param {string} str - The input string to convert.
 * @returns {string} - The `felt252` representation of the input string.
 */
export function strTofelt252Felt(str: string): string {
  const encoder = new TextEncoder();
  const strB = encoder.encode(str);
  return BigInt(
    strB.reduce((memo, byte) => {
      memo += byte.toString(16)
      return memo
    }, '0x'),
  ).toString()
}

/**
 * Extracts the names of all components from a manifest.
 *
 * @param {any} manifest - The input manifest containing component details.
 * @returns {any} - An array containing the names of all components.
 */
export function getAllComponentNames(manifest: any): any {
  return manifest.components.map((component: any) => component.name);
}

/**
 * Extracts the names of all components from a manifest and converts them to `felt252` representation.
 *
 * @param {any} manifest - The input manifest containing component details.
 * @returns {any} - An array containing the `felt252` representation of component names.
 */
export function getAllComponentNamesAsFelt(manifest: any): any {
  return manifest.components.map((component: any) => strTofelt252Felt(component.name));
}

/**
 * Extracts the names of all systems from a manifest.
 *
 * @param {any} manifest - The input manifest containing system details.
 * @returns {any} - An array containing the names of all systems.
 */
export function getAllSystemNames(manifest: any): any {
  return manifest.systems.map((system: any) => system.name);
}

/**
 * Extracts the names of all systems from a manifest and converts them to `felt252` representation.
 *
 * @param {any} manifest - The input manifest containing system details.
 * @returns {any} - An array containing the `felt252` representation of system names.
 */
export function getAllSystemNamesAsFelt(manifest: any): any {
  return manifest.systems.map((system: any) => strTofelt252Felt(system.name));
}