const names = {
  'darwin-arm64': 'web-readable.darwin-arm64.node',
  'darwin-x64': 'web-readable.darwin-x64.node',
  'linux-arm64': 'web-readable.linux-arm64-gnu.node',
  'linux-x64': 'web-readable.linux-x64-gnu.node',
  'win32-arm64': 'web-readable.win32-arm64-msvc.node',
  'win32-x64': 'web-readable.win32-x64-msvc.node'
}
const binary = names[`${process.platform}-${process.arch}`]
if (!binary) throw new Error(`Unsupported platform: ${process.platform}-${process.arch}`)
module.exports = require(`./${binary}`)
