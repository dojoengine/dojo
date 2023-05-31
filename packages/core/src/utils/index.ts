export function strToShortStringFelt(str: string): string {
  const strB = Buffer.from(str)
  return BigInt(
    strB.reduce((memo, byte) => {
      memo += byte.toString(16)
      return memo
    }, '0x'),
  ).toString()
}

export function getAllComponentNames(manifest: any): any {
  return manifest.components.map((component: any) => component.name);
}

export function getAllComponentNamesAsFelt(manifest: any): any {
  return manifest.components.map((component: any) => strToShortStringFelt(component.name));
}

export function getAllSystemNames(manifest: any): any {
  return manifest.systems.map((system: any) => system.name);
}

export function getAllSystemNamesAsFelt(manifest: any): any {
  return manifest.systems.map((system: any) => strToShortStringFelt(system.name));
}