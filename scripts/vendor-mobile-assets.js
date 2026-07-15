import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

const source = resolve('node_modules/heic2any/dist/heic2any.js')
const target = resolve('src-tauri/mobile/vendor/heic2any.js')
const checkOnly = process.argv.includes('--check')

const sourceBytes = await readFile(source)

if (checkOnly) {
  const targetBytes = await readFile(target).catch(() => undefined)
  if (!targetBytes?.equals(sourceBytes)) {
    throw new Error('Vendored heic2any.js is missing or differs from heic2any@0.0.4. Run pnpm mobile:vendor.')
  }
}
else {
  await mkdir(dirname(target), { recursive: true })
  await writeFile(target, sourceBytes)
}
