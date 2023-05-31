 export function strToShortStringFelt(str: string): string {
  const strB = Buffer.from(str)
  return BigInt(
    strB.reduce((memo, byte) => {
      memo += byte.toString(16)
      return memo
    }, '0x'),
  ).toString()
}