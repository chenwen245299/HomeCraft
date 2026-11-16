// Render our editable SVG and build a transparent macOS icon. Requires Node.js + sharp.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
async function main() {
  const assets = path.join(__dirname, '../assets/app-icon');
  const source = path.join(assets, 'homecraft.svg');
  const chunks = [];
  for (const [type, size] of [['icp4',16],['icp5',32],['icp6',64],['ic07',128],['ic08',256],['ic09',512],['ic10',1024]]) {
    const png = await sharp(source, {density: 192}).resize(size, size).png().toBuffer();
    if (size === 1024 || size === 128) fs.writeFileSync(path.join(assets, `homecraft-${size}.png`), png);
    const header = Buffer.alloc(8);
    header.write(type);
    header.writeUInt32BE(png.length + 8, 4);
    chunks.push(header, png);
  }
  const payload = Buffer.concat(chunks);
  const header = Buffer.alloc(8);
  header.write('icns');
  header.writeUInt32BE(payload.length + 8, 4);
  fs.writeFileSync(path.join(assets, 'HomeCraft.icns'), Buffer.concat([header, payload]));
  // macOS tints this alpha-only template for the menu bar. Supply a Retina-sized
  // source; tray-icon gives NSImage an 18pt logical size independently of pixels.
  for (const size of [22, 44, 176]) {
    await sharp(path.join(assets, 'homecraft-tray.svg'), {density: 576})
      .resize(size, size).png().toFile(path.join(assets, `homecraft-tray-${size}.png`));
  }
  console.log('Rendered HomeCraft app icon and upright menu-bar templates.');
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
