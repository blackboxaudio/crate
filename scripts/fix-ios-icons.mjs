#!/usr/bin/env node
/**
 * Regenerate the iOS app-icon set (`src-tauri/icons/{dev,staging,prod}/ios/*.png`) as
 * full-bleed, opaque squares.
 *
 * Why this exists: the per-channel masters (`icons/{channel}/icon.png` + `icon.icns`) are
 * macOS-style icons — a rounded squircle with a transparent margin and an alpha channel.
 * That shape is correct for the macOS `.icns` (the OS expects the rounded, padded artwork),
 * but it is WRONG for iOS: iOS forbids alpha in app icons, flattens any transparency to
 * white, and then applies its OWN squircle mask. Feeding it the rounded-with-margin master
 * produces a visible white border around the artwork on the Home Screen.
 *
 * `tauri icon icons/{channel}/icon.png` generates every platform (desktop + ios + android)
 * from that single macOS-shaped master, so it re-introduces the bug for iOS each time it runs.
 * Run THIS script afterwards to correct the iOS set.
 *
 * What it does, per channel:
 *   1. Extracts the highest-resolution representation (1024px) from `icon.icns` via `iconutil`.
 *   2. Crops to the squircle's bounding box (drops the transparent margin so the artwork
 *      fills the frame the way an iOS icon should).
 *   3. Bleeds each row's edge colour outward to fill the rounded-corner triangles, then drops
 *      the alpha channel entirely — yielding a full-bleed, fully-opaque RGB square. iOS then
 *      applies its own corner mask cleanly, with no white border.
 *   4. Resizes that corrected master into every size already present in `icons/{channel}/ios/`
 *      and writes a 1024px `icons/{channel}/icon-ios.png` as the documented source of truth.
 *
 * Zero npm deps: PNG codec is hand-rolled on `node:zlib`; `iconutil` is ambient on macOS.
 *
 * Usage: node scripts/fix-ios-icons.mjs
 */
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { deflateSync, inflateSync } from 'node:zlib'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const iconsRoot = resolve(repoRoot, 'src-tauri/icons')
const CHANNELS = ['dev', 'staging', 'prod']
const ALPHA_T = 16 // alpha above this counts as "opaque artwork"

// ---- CRC32 (for PNG chunk checksums) ----
const CRC_TABLE = (() => {
	const t = new Uint32Array(256)
	for (let n = 0; n < 256; n++) {
		let c = n
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
		t[n] = c >>> 0
	}
	return t
})()
function crc32(buf) {
	let c = 0xffffffff
	for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8)
	return (c ^ 0xffffffff) >>> 0
}

/** Decode an 8-bit, non-interlaced PNG (colour type 2 RGB or 6 RGBA). Returns {w,h,ch,data}. */
function decodePng(buf) {
	if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG')
	let off = 8
	let w, h, ct
	const idat = []
	while (off < buf.length) {
		const len = buf.readUInt32BE(off)
		const type = buf.toString('ascii', off + 4, off + 8)
		const data = buf.subarray(off + 8, off + 8 + len)
		if (type === 'IHDR') {
			w = data.readUInt32BE(0)
			h = data.readUInt32BE(4)
			const bitDepth = data[8]
			ct = data[9]
			const interlace = data[12]
			if (bitDepth !== 8 || interlace !== 0 || (ct !== 2 && ct !== 6))
				throw new Error(`unsupported PNG (bitDepth=${bitDepth} colourType=${ct} interlace=${interlace})`)
		} else if (type === 'IDAT') idat.push(Buffer.from(data))
		else if (type === 'IEND') break
		off += 12 + len
	}
	const raw = inflateSync(Buffer.concat(idat))
	const ch = ct === 6 ? 4 : 3
	const stride = w * ch
	const out = Buffer.alloc(h * stride)
	let pos = 0
	const paeth = (a, b, c) => {
		const p = a + b - c
		const pa = Math.abs(p - a)
		const pb = Math.abs(p - b)
		const pc = Math.abs(p - c)
		return pa <= pb && pa <= pc ? a : pb <= pc ? b : c
	}
	for (let y = 0; y < h; y++) {
		const ft = raw[pos++]
		for (let x = 0; x < stride; x++) {
			const v = raw[pos++]
			const a = x >= ch ? out[y * stride + x - ch] : 0
			const b = y > 0 ? out[(y - 1) * stride + x] : 0
			const c = x >= ch && y > 0 ? out[(y - 1) * stride + x - ch] : 0
			let r
			if (ft === 0) r = v
			else if (ft === 1) r = v + a
			else if (ft === 2) r = v + b
			else if (ft === 3) r = v + ((a + b) >> 1)
			else if (ft === 4) r = v + paeth(a, b, c)
			else throw new Error('bad PNG filter ' + ft)
			out[y * stride + x] = r & 0xff
		}
	}
	return { w, h, ch, data: out }
}

/** Encode a {w,h,data} RGB image as an 8-bit colour-type-2 (no alpha) PNG. */
function encodePng({ w, h, data }) {
	const stride = w * 3
	const raw = Buffer.alloc((stride + 1) * h)
	for (let y = 0; y < h; y++) {
		raw[y * (stride + 1)] = 0 // filter: None
		data.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride)
	}
	const comp = deflateSync(raw, { level: 9 })
	const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
	const parts = [sig]
	const chunk = (type, body) => {
		const len = Buffer.alloc(4)
		len.writeUInt32BE(body.length, 0)
		const t = Buffer.from(type, 'ascii')
		const crc = Buffer.alloc(4)
		crc.writeUInt32BE(crc32(Buffer.concat([t, body])), 0)
		parts.push(len, t, body, crc)
	}
	const ihdr = Buffer.alloc(13)
	ihdr.writeUInt32BE(w, 0)
	ihdr.writeUInt32BE(h, 4)
	ihdr[8] = 8 // bit depth
	ihdr[9] = 2 // colour type: truecolour (RGB, no alpha)
	chunk('IHDR', ihdr)
	chunk('IDAT', comp)
	chunk('IEND', Buffer.alloc(0))
	return Buffer.concat(parts)
}

/**
 * Convert the macOS-style rounded-with-margin RGBA master into a full-bleed opaque RGB square.
 *
 * The squircle carries a thin (~2-3px) perimeter outline stroke whose colour contrasts with the
 * fill (light on the dark/teal channels, dark on the white one). A naive "bleed the outermost
 * opaque pixel outward" fills the corners with that OUTLINE colour, which reintroduces a border.
 * Instead we:
 *   1. Crop to the squircle's alpha bounding box.
 *   2. Erode the opaque mask by a few pixels to drop the outline stroke, yielding the true
 *      interior (background fill + centred artwork).
 *   3. Bleed that interior outward — horizontally, then vertically — so every margin / rounded-
 *      corner pixel takes the nearest *background* colour (preserving the subtle vertical
 *      gradient), never the outline. Alpha is dropped entirely.
 */
function flattenFullBleed({ w, h, ch, data }) {
	const opaque = (x, y) => (ch === 4 ? data[(y * w + x) * 4 + 3] > ALPHA_T : true)
	let minx = w
	let miny = h
	let maxx = -1
	let maxy = -1
	for (let y = 0; y < h; y++)
		for (let x = 0; x < w; x++)
			if (opaque(x, y)) {
				if (x < minx) minx = x
				if (x > maxx) maxx = x
				if (y < miny) miny = y
				if (y > maxy) maxy = y
			}
	if (maxx < 0) {
		minx = 0
		miny = 0
		maxx = w - 1
		maxy = h - 1
	}
	const cw = maxx - minx + 1
	const chh = maxy - miny + 1

	// Erosion radius: enough to strip the thin outline, far less than the artwork's edge padding.
	const erode = Math.max(6, Math.round(0.012 * Math.max(cw, chh)))
	const op = new Uint8Array(cw * chh)
	for (let y = 0; y < chh; y++) for (let x = 0; x < cw; x++) op[y * cw + x] = opaque(minx + x, miny + y) ? 1 : 0
	// Separable binary erosion with a square structuring element (rows, then columns).
	const rowEr = new Uint8Array(cw * chh)
	for (let y = 0; y < chh; y++)
		for (let x = 0; x < cw; x++) {
			let keep = 1
			for (let k = -erode; k <= erode; k++) {
				const xx = x + k
				if (xx < 0 || xx >= cw || !op[y * cw + xx]) {
					keep = 0
					break
				}
			}
			rowEr[y * cw + x] = keep
		}
	const interior = new Uint8Array(cw * chh)
	for (let x = 0; x < cw; x++)
		for (let y = 0; y < chh; y++) {
			let keep = 1
			for (let k = -erode; k <= erode; k++) {
				const yy = y + k
				if (yy < 0 || yy >= chh || !rowEr[yy * cw + x]) {
					keep = 0
					break
				}
			}
			interior[y * cw + x] = keep
		}

	const out = Buffer.alloc(cw * chh * 3)
	const filled = new Uint8Array(cw * chh)
	const fromSrc = (ox, oy) => {
		const s = ((miny + oy) * w + (minx + ox)) * ch
		const o = (oy * cw + ox) * 3
		out[o] = data[s]
		out[o + 1] = data[s + 1]
		out[o + 2] = data[s + 2]
		filled[oy * cw + ox] = 1
	}
	const fromOut = (ox, oy, fx, fy) => {
		const so = (fy * cw + fx) * 3
		const o = (oy * cw + ox) * 3
		out[o] = out[so]
		out[o + 1] = out[so + 1]
		out[o + 2] = out[so + 2]
		filled[oy * cw + ox] = 1
	}
	// Keep interior pixels verbatim.
	for (let y = 0; y < chh; y++) for (let x = 0; x < cw; x++) if (interior[y * cw + x]) fromSrc(x, y)
	// Horizontal bleed: extend each row's interior edge colour out to the frame.
	for (let y = 0; y < chh; y++) {
		let first = -1
		let last = -1
		for (let x = 0; x < cw; x++)
			if (filled[y * cw + x]) {
				if (first < 0) first = x
				last = x
			}
		if (first < 0) continue
		for (let x = 0; x < first; x++) fromOut(x, y, first, y)
		for (let x = last + 1; x < cw; x++) fromOut(x, y, last, y)
	}
	// Vertical bleed: fill any rows that had no interior (top/bottom strips) from nearest filled.
	for (let x = 0; x < cw; x++) {
		let first = -1
		let last = -1
		for (let y = 0; y < chh; y++)
			if (filled[y * cw + x]) {
				if (first < 0) first = y
				last = y
			}
		if (first < 0) continue
		for (let y = 0; y < first; y++) fromOut(x, y, x, first)
		for (let y = last + 1; y < chh; y++) fromOut(x, y, x, last)
	}
	return { w: cw, h: chh, data: out }
}

/** Resize RGB: area-average when downscaling, bilinear when upscaling. */
function resizeRGB({ w: sw, h: sh, data }, tw, th) {
	const out = Buffer.alloc(tw * th * 3)
	const upscaling = tw > sw || th > sh
	if (upscaling) {
		for (let ty = 0; ty < th; ty++) {
			const fy = ((ty + 0.5) * sh) / th - 0.5
			let y0 = Math.floor(fy)
			let wy = fy - y0
			if (y0 < 0) {
				y0 = 0
				wy = 0
			}
			const y1 = Math.min(y0 + 1, sh - 1)
			for (let tx = 0; tx < tw; tx++) {
				const fx = ((tx + 0.5) * sw) / tw - 0.5
				let x0 = Math.floor(fx)
				let wx = fx - x0
				if (x0 < 0) {
					x0 = 0
					wx = 0
				}
				const x1 = Math.min(x0 + 1, sw - 1)
				const o = (ty * tw + tx) * 3
				for (let c = 0; c < 3; c++) {
					const p00 = data[(y0 * sw + x0) * 3 + c]
					const p01 = data[(y0 * sw + x1) * 3 + c]
					const p10 = data[(y1 * sw + x0) * 3 + c]
					const p11 = data[(y1 * sw + x1) * 3 + c]
					const top = p00 + (p01 - p00) * wx
					const bot = p10 + (p11 - p10) * wx
					out[o + c] = Math.round(top + (bot - top) * wy)
				}
			}
		}
	} else {
		for (let ty = 0; ty < th; ty++) {
			const sy0 = Math.floor((ty * sh) / th)
			const sy1 = Math.max(sy0 + 1, Math.floor(((ty + 1) * sh) / th))
			for (let tx = 0; tx < tw; tx++) {
				const sx0 = Math.floor((tx * sw) / tw)
				const sx1 = Math.max(sx0 + 1, Math.floor(((tx + 1) * sw) / tw))
				let r = 0
				let g = 0
				let b = 0
				let n = 0
				for (let yy = sy0; yy < sy1; yy++)
					for (let xx = sx0; xx < sx1; xx++) {
						const s = (yy * sw + xx) * 3
						r += data[s]
						g += data[s + 1]
						b += data[s + 2]
						n++
					}
				const o = (ty * tw + tx) * 3
				out[o] = Math.round(r / n)
				out[o + 1] = Math.round(g / n)
				out[o + 2] = Math.round(b / n)
			}
		}
	}
	return { w: tw, h: th, data: out }
}

for (const channel of CHANNELS) {
	const chDir = join(iconsRoot, channel)
	const iosDir = join(chDir, 'ios')
	const work = mkdtempSync(join(tmpdir(), `crate-icns-${channel}-`))
	try {
		const iconset = join(work, 'icon.iconset')
		execFileSync('iconutil', ['-c', 'iconset', join(chDir, 'icon.icns'), '-o', iconset])
		const src = decodePng(readFileSync(join(iconset, 'icon_512x512@2x.png')))
		const corrected = flattenFullBleed(src)
		writeFileSync(join(chDir, 'icon-ios.png'), encodePng(resizeRGB(corrected, 1024, 1024)))
		const targets = readdirSync(iosDir).filter((n) => n.endsWith('.png'))
		for (const name of targets) {
			const { w } = decodePng(readFileSync(join(iosDir, name)))
			writeFileSync(join(iosDir, name), encodePng(resizeRGB(corrected, w, w)))
		}
		console.log(
			`[fix-ios-icons] ${channel}: ${targets.length} icons regenerated from ${corrected.w}px master (full-bleed, no alpha) + icon-ios.png`
		)
	} finally {
		rmSync(work, { recursive: true, force: true })
	}
}
