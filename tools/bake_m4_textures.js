// Generates the two tileable maps crates/client/assets/models/m4a1/mesh/
// references. The model is CC-BY from Sketchfab and shipped with no textures
// at all, only four flat materials, two of which were named "Grey" while
// holding a base colour of 0.01 -- black, not grey.
//
// Tileable rather than an atlas on purpose: every part of this mesh unwraps to
// its own full 0..1 square, so there is no shared layout to paint into. A few
// parts (the carry handle) have collapsed UVs and will read one texel, which
// is why the maps stay subtle enough to survive being a flat colour.
//
//   node tools/bake_m4_textures.js
//
// Writes gunmetal_base.png (sRGB albedo) and gunmetal_mr.png (glTF packs
// roughness in G and metallic in B).

const fs = require("fs");
const zlib = require("zlib");
const path = require("path");

const SIZE = 512;
const OUT = path.join(__dirname, "../crates/client/assets/models/m4a1/mesh");

const fract = (x) => x - Math.floor(x);
const mix = (a, b, t) => a + (b - a) * t;
const clamp01 = (x) => Math.min(1, Math.max(0, x));

// Wrapped lattice hash, so every octave repeats inside the tile and the edges
// meet. An unwrapped hash gives a seam down the middle of every part.
// Each axis wraps at its own period, so a pattern can be stretched hard along
// one axis and still meet itself at both edges.
function hash2(x, y, px, py, seed) {
  const ix = ((x % px) + px) % px;
  const iy = ((y % py) + py) % py;
  return fract(Math.sin(ix * 127.1 + iy * 311.7 + seed * 74.7) * 43758.5453);
}

function noise2(u, v, px, py, seed) {
  const x = u * px;
  const y = v * py;
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const fx = x - x0;
  const fy = y - y0;
  const ux = fx * fx * (3 - 2 * fx);
  const uy = fy * fy * (3 - 2 * fy);
  const a = hash2(x0, y0, px, py, seed);
  const b = hash2(x0 + 1, y0, px, py, seed);
  const c = hash2(x0, y0 + 1, px, py, seed);
  const d = hash2(x0 + 1, y0 + 1, px, py, seed);
  return mix(mix(a, b, ux), mix(c, d, ux), uy);
}

function fbm(u, v, px, py, octaves, seed) {
  let sum = 0;
  let amp = 0.5;
  for (let o = 0; o < octaves; o++) {
    sum += amp * noise2(u, v, px << o, py << o, seed + o);
    amp *= 0.5;
  }
  return sum;
}

// Machining marks: a lattice stretched hard along one axis reads as fine
// parallel tooling lines, which is most of what makes a gun body look
// manufactured rather than moulded.
function brushed(u, v, seed) {
  return fbm(u, v, 3, 96, 3, seed);
}

const base = Buffer.alloc(SIZE * SIZE * 3);
const mr = Buffer.alloc(SIZE * SIZE * 3);

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const u = x / SIZE;
    const v = y / SIZE;

    const mottle = fbm(u, v, 4, 4, 4, 1.0) - 0.5;
    const marks = brushed(u, v, 7.0) - 0.5;
    const grit = fbm(u, v, 64, 64, 2, 13.0) - 0.5;

    // Albedo near white: the material's own baseColorFactor sets how dark the
    // part is, so this only carries variation. Bake the darkness in here and
    // every part would go black twice over.
    const wear = clamp01(0.88 + mottle * 0.22 + marks * 0.10 + grit * 0.06);
    const warm = clamp01(wear * 1.01);
    const cool = clamp01(wear * 0.985);
    let o = (y * SIZE + x) * 3;
    base[o] = Math.round(warm * 255);
    base[o + 1] = Math.round(wear * 255);
    base[o + 2] = Math.round(cool * 255);

    // Rougher where it is pitted, smoother along the tooling marks, so the
    // highlight breaks up instead of sliding evenly over the whole part.
    const rough = clamp01(0.42 + grit * 0.5 - marks * 0.28 + mottle * 0.14);
    mr[o] = 0;
    mr[o + 1] = Math.round(rough * 255);
    mr[o + 2] = 255;
  }
}

function png(pixels, width, height) {
  const stride = width * 3;
  const filtered = Buffer.alloc(height * (1 + stride));
  for (let y = 0; y < height; y++) {
    filtered[y * (1 + stride)] = 0;
    pixels.copy(filtered, y * (1 + stride) + 1, y * stride, (y + 1) * stride);
  }
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  const crc = (buf) => {
    let c = -1;
    for (const v of buf) c = table[(c ^ v) & 255] ^ (c >>> 8);
    return (c ^ -1) >>> 0;
  };
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const c = Buffer.alloc(4);
    c.writeUInt32BE(crc(td));
    return Buffer.concat([len, td, c]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(filtered, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

fs.writeFileSync(path.join(OUT, "gunmetal_base.png"), png(base, SIZE, SIZE));
fs.writeFileSync(path.join(OUT, "gunmetal_mr.png"), png(mr, SIZE, SIZE));
console.log(`gunmetal_base.png + gunmetal_mr.png, ${SIZE}x${SIZE}`);
